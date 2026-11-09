//! The SWF container and the `ActionScript` 1/2 subset that FFR levels use to write their arrays, as R^3 reads them (`Beatbox.parseBeatbox`):
//! each `DoAction` tag runs on its own fresh stack, variables and registers; the first tag that leaves `_root.<name>` set wins. No
//! jump, call or loop exists in the subset, so a tag runs in one pass over its bytes.

use std::collections::HashMap;
use std::io::Read;
use std::rc::Rc;

use flate2::read::ZlibDecoder;

/// Uncompressed movies above this size are refused (a level with its MP3 is a few megabytes).
pub(super) const MAX_MOVIE: usize = 64 * 1024 * 1024;
const TAG_END: u16 = 0;
const TAG_DO_ACTION: u16 = 12;

/// A value of the stack.
#[derive(Clone, Debug)]
pub(super) enum Value {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    Str(Rc<str>),
    Array(Rc<[Value]>),
    /// An object of [`Machine::objects`].
    Object(usize),
}

impl Value {
    pub(super) fn number(&self) -> Option<f64> {
        match self {
            Self::Number(number) => Some(*number),
            _ => None,
        }
    }

    pub(super) fn text(&self) -> Option<&str> {
        match self {
            Self::Str(text) => Some(text),
            _ => None,
        }
    }

    /// The name a value stands for as a variable or member name.
    fn key(&self) -> Rc<str> {
        match self {
            Self::Str(text) => Rc::clone(text),
            Self::Number(number) => number.to_string().into(),
            Self::Bool(value) => value.to_string().into(),
            Self::Null => "null".into(),
            Self::Undefined | Self::Array(_) | Self::Object(_) => "undefined".into(),
        }
    }
}

/// Why a movie can not be read at all (a tag that fails to run is skipped, as R^3 does).
#[derive(Debug)]
pub(super) enum SwfError {
    NotSwf,
    Lzma,
    TooLarge,
    Truncated,
}

impl std::fmt::Display for SwfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotSwf => "not a Flash movie (FWS/CWS)",
            Self::Lzma => "LZMA-compressed movie (ZWS) not supported",
            Self::TooLarge => "movie too large",
            Self::Truncated => "truncated movie",
        })
    }
}

/// The movie's tags after the header, uncompressed.
pub(super) fn body(data: &[u8]) -> Result<Vec<u8>, SwfError> {
    let (Some(signature), Some(length)) = (data.get(..3), data.get(4..8)) else {
        return Err(SwfError::NotSwf);
    };
    let length = u32::from_le_bytes(length.try_into().expect("4 bytes")) as usize;
    if length > MAX_MOVIE {
        return Err(SwfError::TooLarge);
    }
    match signature {
        b"FWS" => Ok(data[8..].to_vec()),
        b"CWS" => {
            let mut body = Vec::with_capacity(length.saturating_sub(8));
            ZlibDecoder::new(&data[8..]).take(MAX_MOVIE as u64 + 1).read_to_end(&mut body).map_err(|_| SwfError::Truncated)?;
            if body.len() > MAX_MOVIE {
                return Err(SwfError::TooLarge);
            }
            Ok(body)
        }
        b"ZWS" => Err(SwfError::Lzma),
        _ => Err(SwfError::NotSwf),
    }
}

/// The payloads of the `DoAction` tags of the main timeline (sprites are not entered, as R^3 does not).
pub(super) fn do_actions(body: &[u8]) -> Result<Vec<&[u8]>, SwfError> {
    // Frame size: a RECT of 5 + 4 × n bits, then the frame rate and count (2 bytes each).
    let bits = usize::from(*body.first().ok_or(SwfError::Truncated)? >> 3);
    let mut at = (5 + 4 * bits).div_ceil(8) + 4;
    let mut actions = Vec::new();
    while at + 2 <= body.len() {
        let code_and_length = u16::from_le_bytes([body[at], body[at + 1]]);
        at += 2;
        let (code, mut length) = (code_and_length >> 6, usize::from(code_and_length & 0x3F));
        if length == 0x3F {
            let long = body.get(at..at + 4).ok_or(SwfError::Truncated)?;
            length = u32::from_le_bytes(long.try_into().expect("4 bytes")) as usize;
            at += 4;
        }
        let payload = body.get(at..at.saturating_add(length)).ok_or(SwfError::Truncated)?;
        at += length;
        match code {
            TAG_END => break,
            TAG_DO_ACTION => actions.push(payload),
            _ => {}
        }
    }
    Ok(actions)
}

/// One run of a `DoAction` tag.
struct Machine {
    stack: Vec<Value>,
    variables: HashMap<Rc<str>, Value>,
    objects: Vec<HashMap<Rc<str>, Value>>,
    registers: Vec<Value>,
    pool: Vec<Rc<str>>,
}

/// Runs one `DoAction` payload and returns its `_root.<name>` array; `None` when the tag fails to run or does not set it.
pub(super) fn root_array(actions: &[u8], name: &str) -> Option<Rc<[Value]>> {
    let mut machine = Machine { stack: Vec::new(), variables: HashMap::new(), objects: Vec::new(), registers: vec![Value::Undefined; 256], pool: Vec::new() };
    machine.run(actions)?;
    let Some(Value::Object(root)) = machine.variables.get("_root") else { return None };
    match machine.objects[*root].get(name) {
        Some(Value::Array(array)) => Some(Rc::clone(array)),
        _ => None,
    }
}

/// A NUL-terminated string at the start of `bytes` (SWF 6 and later write UTF-8) and the bytes after it.
fn string(bytes: &[u8]) -> Option<(Rc<str>, &[u8])> {
    let end = memchr::memchr(0, bytes)?;
    Some((String::from_utf8_lossy(&bytes[..end]).into(), &bytes[end + 1..]))
}

impl Machine {
    fn pop(&mut self) -> Option<Value> {
        self.stack.pop()
    }

    fn object(&mut self) -> usize {
        self.objects.push(HashMap::new());
        self.objects.len() - 1
    }

    /// A variable (`object` `None`) or a member, as `ActionScript` reads it here: a name not set yet becomes a new empty object.
    fn get_or_create(&mut self, object: Option<usize>, name: Rc<str>) -> Value {
        let found = match object {
            None => self.variables.get(&name),
            Some(object) => self.objects[object].get(&name),
        };
        if let Some(value) = found {
            return value.clone();
        }
        let value = Value::Object(self.object());
        match object {
            None => self.variables.insert(name, value.clone()),
            Some(object) => self.objects[object].insert(name, value.clone()),
        };
        value
    }

    fn run(&mut self, mut code: &[u8]) -> Option<()> {
        loop {
            let (&action, rest) = code.split_first()?;
            let (payload, rest) = if action >= 0x80 {
                let length = usize::from(u16::from_le_bytes([*rest.first()?, *rest.get(1)?]));
                let rest = &rest[2..];
                (rest.get(..length)?, &rest[length..])
            } else {
                (&[][..], rest)
            };
            code = rest;
            match action {
                0x00 => return Some(()),
                0x88 => self.constant_pool(payload)?,
                0x96 => self.push(payload)?,
                0x17 => {
                    self.pop()?;
                }
                0x4C => {
                    let top = self.stack.last()?.clone();
                    self.stack.push(top);
                }
                0x87 => self.registers[usize::from(*payload.first()?)] = self.stack.last()?.clone(),
                0x1C => {
                    let name = self.pop()?.key();
                    let value = self.get_or_create(None, name);
                    self.stack.push(value);
                }
                0x1D => {
                    let value = self.pop()?;
                    let name = self.pop()?.key();
                    self.variables.insert(name, value);
                }
                0x42 => {
                    let count = self.pop()?.number()?;
                    // A count that is not a whole number the stack holds fails the tag.
                    #[allow(clippy::cast_precision_loss)] // a stack is far below 2^52 values
                    let depth = self.stack.len() as f64;
                    if !(0.0..=depth).contains(&count) || count.fract() != 0.0 {
                        return None;
                    }
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // checked just above
                    let first = self.stack.len() - count as usize;
                    // The first popped is the first element.
                    let array: Rc<[Value]> = self.stack.drain(first..).rev().collect();
                    self.stack.push(Value::Array(array));
                }
                0x4E => {
                    let name = self.pop()?.key();
                    let Value::Object(object) = self.pop()? else { return None };
                    let value = self.get_or_create(Some(object), name);
                    self.stack.push(value);
                }
                0x4F => {
                    let value = self.pop()?;
                    let name = self.pop()?.key();
                    let Value::Object(object) = self.pop()? else { return None };
                    self.objects[object].insert(name, value);
                }
                _ => {}
            }
        }
    }

    fn constant_pool(&mut self, payload: &[u8]) -> Option<()> {
        let count = u16::from_le_bytes([*payload.first()?, *payload.get(1)?]);
        let mut rest = &payload[2..];
        self.pool.clear();
        for _ in 0..count {
            let (text, after) = string(rest)?;
            self.pool.push(text);
            rest = after;
        }
        Some(())
    }

    fn push(&mut self, mut payload: &[u8]) -> Option<()> {
        while let Some((&kind, rest)) = payload.split_first() {
            let (value, size) = match kind {
                0 => {
                    let (text, after) = string(rest)?;
                    payload = after;
                    self.stack.push(Value::Str(text));
                    continue;
                }
                1 => (Value::Number(f64::from(f32::from_le_bytes(rest.get(..4)?.try_into().ok()?))), 4),
                2 => (Value::Null, 0),
                3 => (Value::Undefined, 0),
                4 => (self.registers[usize::from(*rest.first()?)].clone(), 1),
                5 => (Value::Bool(*rest.first()? != 0), 1),
                // A double is written as its high 32 bits then its low 32 bits, each little-endian.
                6 => {
                    let bytes = rest.get(..8)?;
                    let bits = (u64::from(u32::from_le_bytes(bytes[..4].try_into().ok()?)) << 32) | u64::from(u32::from_le_bytes(bytes[4..].try_into().ok()?));
                    (Value::Number(f64::from_bits(bits)), 8)
                }
                7 => (Value::Number(f64::from(i32::from_le_bytes(rest.get(..4)?.try_into().ok()?))), 4),
                8 => (self.pool.get(usize::from(*rest.first()?)).map_or(Value::Undefined, |text| Value::Str(Rc::clone(text))), 1),
                9 => {
                    let index = usize::from(u16::from_le_bytes([*rest.first()?, *rest.get(1)?]));
                    (self.pool.get(index).map_or(Value::Undefined, |text| Value::Str(Rc::clone(text))), 2)
                }
                _ => return None,
            };
            self.stack.push(value);
            payload = &rest[size..];
        }
        Some(())
    }
}
