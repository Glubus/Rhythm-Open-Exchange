#![warn(clippy::pedantic)]
//! A small reader of Standard MIDI Files: the header, the track chunks and the events of a track.
//!
//! Nothing here knows ROX or Clone Hero. Only what `mid.rs` needs is decoded: note on/off, tempo, time signature,
//! text/track-name meta events and `SysEx`; every other event is read (to stay in step) and skipped.
//!
//! Input is untrusted. The file is capped ([`MAX_FILE_BYTES`]), so are the chunks walked ([`MAX_CHUNKS`]) and the events
//! of a track ([`MAX_EVENTS_PER_TRACK`]); every length is checked against the bytes that remain before it is used, and
//! nothing is allocated from a length read in the file.
//!
//! Notes on the format (`vsrg-format-docs/vsrg/clone-hero/mid`, `GuitarGame_ChartFormats` `mid-format`):
//!
//! - Format 0 and 1 are read; format 2 and SMPTE time division are errors (the notes say they are not supported).
//! - Delta times and lengths are variable-length quantities of at most 4 bytes.
//! - Running status: a data byte where a status byte is expected repeats the last channel status. The standard resets
//!   it after a `SysEx` or meta event; charts exist that do not reset it (the notes list it as a known violation), so it is
//!   NOT reset here: every correct file still reads the same.
//! - `SysEx` may hold bytes above `0x7F` (the notes: `0xFF` is common): its length is what counts, never its content.
//! - Chunks of unknown type are skipped; the track count of the header is only metadata (the notes say so).
//! - `0xFF 0x2F` ends a track: what follows it in the chunk is not read.

/// Largest file read, in bytes. Real charts are a few hundred KiB.
pub const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
/// Chunks walked before the file is refused.
pub const MAX_CHUNKS: usize = 1024;
/// Events of one track read before the track is refused.
pub const MAX_EVENTS_PER_TRACK: usize = 4_000_000;

/// The header and the bodies of the track chunks.
#[derive(Debug)]
pub struct Smf<'a> {
    /// 0 or 1.
    pub format: u16,
    /// Ticks per quarter note, 1..=32767.
    pub ticks_per_quarter: u32,
    /// Body of every `MTrk` chunk, in file order.
    pub tracks: Vec<&'a [u8]>,
}

fn be16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*data.get(at)?, *data.get(at + 1)?]))
}

fn be32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *data.get(at)?,
        *data.get(at + 1)?,
        *data.get(at + 2)?,
        *data.get(at + 3)?,
    ]))
}

/// Reads the header and slices the track chunks.
///
/// # Errors
/// The file is larger than [`MAX_FILE_BYTES`], has no `MThd` header (or a cut one), has format 2 or an SMPTE division
/// or a division of 0, a chunk longer than the file, or more than [`MAX_CHUNKS`] chunks.
pub fn parse(data: &[u8]) -> Result<Smf<'_>, String> {
    if data.len() > MAX_FILE_BYTES {
        return Err(format!("{} bytes, more than {MAX_FILE_BYTES}", data.len()));
    }
    if data.get(..4) != Some(b"MThd") {
        return Err("no MThd header".into());
    }
    let header_len = be32(data, 4).ok_or("header cut short")? as usize;
    if header_len < 6 {
        return Err(format!("header of {header_len} bytes, at least 6 needed"));
    }
    let format = be16(data, 8).ok_or("header cut short")?;
    let division = be16(data, 12).ok_or("header cut short")?;
    if format > 1 {
        return Err(format!("format {format} is not supported (0 and 1 only)"));
    }
    if division & 0x8000 != 0 {
        return Err("SMPTE time division is not supported".into());
    }
    if division == 0 {
        return Err("a division of 0 ticks per quarter note".into());
    }
    let mut at = 8usize
        .checked_add(header_len)
        .filter(|&end| end <= data.len())
        .ok_or("header runs past the end")?;
    let mut tracks = Vec::new();
    let mut chunks = 0usize;
    while data.len() - at >= 8 {
        chunks += 1;
        if chunks > MAX_CHUNKS {
            return Err(format!("more than {MAX_CHUNKS} chunks"));
        }
        let len = be32(data, at + 4).ok_or("chunk cut short")? as usize;
        let start = at + 8;
        let end = start
            .checked_add(len)
            .filter(|&end| end <= data.len())
            .ok_or("chunk runs past the end")?;
        if &data[at..at + 4] == b"MTrk" {
            tracks.push(&data[start..end]);
        }
        at = end;
    }
    Ok(Smf {
        format,
        ticks_per_quarter: u32::from(division),
        tracks,
    })
}

/// An event of a track, reduced to what the decoder reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event<'a> {
    /// A note on with a velocity above 0.
    NoteOn { channel: u8, key: u8 },
    /// A note off, or a note on with velocity 0.
    NoteOff { channel: u8, key: u8 },
    /// `FF 51 03`: microseconds per quarter note (may be 0).
    Tempo(u32),
    /// `FF 58`: numerator and the power of 2 of the denominator.
    TimeSignature { numerator: u8, exponent: u8 },
    /// A tempo or time signature meta event whose length is not the one the format gives.
    MalformedSync,
    /// `FF 03`.
    TrackName(&'a [u8]),
    /// `FF 01`..`FF 0F` other than the track name.
    Text(&'a [u8]),
    /// `F0`: the bytes after the length (the closing `F7` included).
    SysEx(&'a [u8]),
    /// `FF 2F`: the iterator stops after it.
    EndOfTrack,
    /// Anything else that is well formed.
    Other,
}

/// The events of a track body with their absolute tick, in file order. After an `Err` (or the end of track) it yields
/// nothing more.
#[derive(Debug)]
pub struct Events<'a> {
    data: &'a [u8],
    pos: usize,
    tick: u64,
    running: Option<u8>,
    count: usize,
    done: bool,
}

impl<'a> Events<'a> {
    #[must_use]
    pub fn new(track: &'a [u8]) -> Self {
        Self {
            data: track,
            pos: 0,
            tick: 0,
            running: None,
            count: 0,
            done: false,
        }
    }

    fn byte(&mut self) -> Result<u8, String> {
        let b = *self.data.get(self.pos).ok_or("event cut short")?;
        self.pos += 1;
        Ok(b)
    }

    /// A variable-length quantity of at most 4 bytes.
    fn vlq(&mut self) -> Result<u32, String> {
        let mut value = 0u32;
        for _ in 0..4 {
            let b = self.byte()?;
            value = (value << 7) | u32::from(b & 0x7F);
            if b & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err("variable-length number longer than 4 bytes".into())
    }

    fn take(&mut self, len: u32) -> Result<&'a [u8], String> {
        let len = len as usize;
        let end = self
            .pos
            .checked_add(len)
            .filter(|&end| end <= self.data.len())
            .ok_or("event runs past the end of its track")?;
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn data_byte(&mut self) -> Result<u8, String> {
        let b = self.byte()?;
        if b >= 0x80 {
            return Err("a status byte where a data byte was expected".into());
        }
        Ok(b)
    }

    fn next_event(&mut self) -> Result<(u64, Event<'a>), String> {
        let delta = self.vlq()?;
        self.tick += u64::from(delta);
        let first = self.byte()?;
        let status = if first >= 0x80 {
            first
        } else {
            self.pos -= 1;
            self.running.ok_or("data byte with no running status")?
        };
        let event = match status {
            0x80..=0xEF => {
                self.running = Some(status);
                let channel = status & 0x0F;
                match status & 0xF0 {
                    0x80 => {
                        let key = self.data_byte()?;
                        self.data_byte()?;
                        Event::NoteOff { channel, key }
                    }
                    0x90 => {
                        let key = self.data_byte()?;
                        let velocity = self.data_byte()?;
                        if velocity == 0 {
                            Event::NoteOff { channel, key }
                        } else {
                            Event::NoteOn { channel, key }
                        }
                    }
                    0xC0 | 0xD0 => {
                        self.data_byte()?;
                        Event::Other
                    }
                    _ => {
                        self.data_byte()?;
                        self.data_byte()?;
                        Event::Other
                    }
                }
            }
            0xF0 => {
                let len = self.vlq()?;
                Event::SysEx(self.take(len)?)
            }
            0xF7 => {
                let len = self.vlq()?;
                self.take(len)?;
                Event::Other
            }
            0xFF => {
                let kind = self.byte()?;
                let len = self.vlq()?;
                let body = self.take(len)?;
                match kind {
                    0x2F => Event::EndOfTrack,
                    0x03 => Event::TrackName(body),
                    0x01..=0x0F => Event::Text(body),
                    0x51 => match body {
                        [a, b, c] => Event::Tempo(u32::from_be_bytes([0, *a, *b, *c])),
                        _ => Event::MalformedSync,
                    },
                    0x58 => match body {
                        [numerator, exponent, ..] => Event::TimeSignature {
                            numerator: *numerator,
                            exponent: *exponent,
                        },
                        _ => Event::MalformedSync,
                    },
                    _ => Event::Other,
                }
            }
            _ => return Err(format!("status byte {status:#04x} is not valid in a file")),
        };
        Ok((self.tick, event))
    }
}

impl<'a> Iterator for Events<'a> {
    type Item = Result<(u64, Event<'a>), String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done || self.pos >= self.data.len() {
            return None;
        }
        self.count += 1;
        if self.count > MAX_EVENTS_PER_TRACK {
            self.done = true;
            return Some(Err(format!(
                "more than {MAX_EVENTS_PER_TRACK} events in a track"
            )));
        }
        let item = self.next_event();
        match &item {
            Ok((_, Event::EndOfTrack)) | Err(_) => self.done = true,
            Ok(_) => {}
        }
        Some(item)
    }
}
