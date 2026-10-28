#![warn(clippy::pedantic)]
//! The chunk walk of an `.ssq`: 12-byte headers (`length`, `type`, `param2`, `param3`, `param4`, little-endian), each chunk
//! `length` bytes long header included, the walk ending at a zero length. Nothing here reads a body.

/// Size of a chunk header.
const HEADER: usize = 12;

/// Chunk types of the format that this decoder reads or names.
pub(super) mod kind {
    pub(crate) const TEMPO: u16 = 1;
    pub(crate) const STEPS: u16 = 3;
    /// Rare metadata chunk, also the chunk of the Hudson (Wii) charts.
    pub(crate) const METADATA: u16 = 9;
    /// Hudson "lane format" chart (Hottest Party 4/5).
    pub(crate) const LANES: u16 = 16;
    /// Mines (DDR World mod).
    pub(crate) const MINES: u16 = 20;
}

/// One chunk: its header fields and the bytes after the header (`length - 12` of them).
#[derive(Debug, Clone, Copy)]
pub(super) struct Chunk<'a> {
    pub offset: usize,
    pub kind: u16,
    pub param2: u16,
    pub param3: u16,
    pub body: &'a [u8],
}

/// The chunks of a file in file order, and why the walk stopped early (`None` when it met the terminator, the end of the
/// file, or the `param2 == 0xFFFF` sentinel the game stops at).
#[derive(Debug, Default)]
pub(super) struct Walk<'a> {
    pub chunks: Vec<Chunk<'a>>,
    pub stopped: Option<String>,
    /// The offset of a chunk whose `param2` is the `0xFFFF` sentinel: the game's lookup stops there, so does the walk.
    pub sentinel_at: Option<usize>,
}

/// Walks the chunks of `data`. A chunk whose length is under 12, not a multiple of 4, or past the end of the file ends the
/// walk (`stopped` says why): the chunks before it are kept. Fewer than four bytes left is the end of the file, as is a zero
/// length (the terminator, followed by any padding).
pub(super) fn walk(data: &[u8]) -> Walk<'_> {
    let mut out = Walk::default();
    let mut pos = 0usize;
    while let Some(length_bytes) = data.get(pos..pos + 4) {
        let length = u32::from_le_bytes([
            length_bytes[0],
            length_bytes[1],
            length_bytes[2],
            length_bytes[3],
        ]);
        if length == 0 {
            break;
        }
        let Ok(length) = usize::try_from(length) else {
            out.stopped = Some(format!(
                "chunk at offset {pos}: length {length} is not usable"
            ));
            break;
        };
        if length < HEADER || !length.is_multiple_of(4) {
            out.stopped = Some(format!(
                "chunk at offset {pos}: length {length} is under 12 or not a multiple of 4"
            ));
            break;
        }
        let Some(chunk) = data.get(pos..pos.saturating_add(length)) else {
            out.stopped = Some(format!(
                "chunk at offset {pos}: length {length} runs past the end of the file ({} bytes)",
                data.len()
            ));
            break;
        };
        let word = |at: usize| u16::from_le_bytes([chunk[at], chunk[at + 1]]);
        let (kind, param2, param3) = (word(4), word(6), word(8));
        if param2 == 0xFFFF {
            out.sentinel_at = Some(pos);
            break;
        }
        out.chunks.push(Chunk {
            offset: pos,
            kind,
            param2,
            param3,
            body: &chunk[HEADER..],
        });
        pos += length;
    }
    out
}

/// The `i32` at index `i` of a little-endian array at the start of `body`. The caller has checked the length.
pub(super) fn i32_at(body: &[u8], i: usize) -> i32 {
    let at = i * 4;
    i32::from_le_bytes([body[at], body[at + 1], body[at + 2], body[at + 3]])
}
