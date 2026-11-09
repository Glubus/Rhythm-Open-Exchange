//! The small part of PNG the chart bitmaps need: 8-bit grey, RGB, palette, grey-alpha or RGBA images, not interlaced; each pixel is
//! told apart as white (or transparent) or not.

use std::io::Read;

use flate2::read::ZlibDecoder;

/// An image as rows of "is this pixel a mark" (anything but white or fully transparent).
pub(super) struct Marks {
    pub width: usize,
    pub rows: Vec<Vec<bool>>,
}

/// Bitmaps above this many pixels are refused (a chart section is 8 × 16).
const MAX_PIXELS: usize = 1 << 20;
const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

pub(super) fn marks(data: &[u8]) -> Result<Marks, String> {
    let mut rest = data.strip_prefix(SIGNATURE).ok_or("not a PNG")?;
    let (mut header, mut palette, mut compressed) = (None, Vec::new(), Vec::new());
    while rest.len() >= 12 {
        let length = u32::from_be_bytes(rest[..4].try_into().expect("4 bytes")) as usize;
        let kind = &rest[4..8];
        let body = rest.get(8..8 + length).ok_or("truncated chunk")?;
        match kind {
            b"IHDR" if body.len() == 13 => header = Some(body.to_vec()),
            b"PLTE" => palette = body.as_chunks::<3>().0.to_vec(),
            b"IDAT" => compressed.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        rest = &rest[(12 + length).min(rest.len())..];
    }
    let header = header.ok_or("no IHDR")?;
    let width = u32::from_be_bytes(header[..4].try_into().expect("4 bytes")) as usize;
    let height = u32::from_be_bytes(header[4..8].try_into().expect("4 bytes")) as usize;
    let (depth, colour, interlace) = (header[8], header[9], header[12]);
    if width == 0 || height == 0 || width.saturating_mul(height) > MAX_PIXELS {
        return Err(format!("{width} × {height} pixels"));
    }
    let channels = match (depth, colour) {
        (8, 0 | 3) => 1,
        (8, 4) => 2,
        (8, 2) => 3,
        (8, 6) => 4,
        _ => return Err(format!("bit depth {depth}, colour type {colour} not read")),
    };
    if interlace != 0 {
        return Err("interlaced image not read".into());
    }
    let stride = width * channels;
    let mut pixels = Vec::with_capacity((stride + 1) * height);
    ZlibDecoder::new(compressed.as_slice()).take(((stride + 1) * height) as u64).read_to_end(&mut pixels).map_err(|e| e.to_string())?;
    if pixels.len() != (stride + 1) * height {
        return Err("truncated image data".into());
    }
    let mut previous = vec![0u8; stride];
    let mut rows = Vec::with_capacity(height);
    for line in pixels.chunks_exact_mut(stride + 1) {
        let (filter, row) = line.split_first_mut().expect("stride + 1 bytes");
        unfilter(*filter, row, &previous, channels)?;
        rows.push(row.chunks_exact(channels).map(|pixel| is_mark(pixel, colour, &palette)).collect());
        previous.copy_from_slice(row);
    }
    Ok(Marks { width, rows })
}

/// Undoes the filter of one scanline (PNG filter types 0 to 4) against the previous one.
fn unfilter(filter: u8, row: &mut [u8], previous: &[u8], channels: usize) -> Result<(), String> {
    for at in 0..row.len() {
        let left = if at >= channels { row[at - channels] } else { 0 };
        let (up, up_left) = (previous[at], if at >= channels { previous[at - channels] } else { 0 });
        let predicted = match filter {
            0 => 0,
            1 => left,
            2 => up,
            3 => left.midpoint(up),
            4 => paeth(left, up, up_left),
            _ => return Err(format!("unknown filter {filter}")),
        };
        row[at] = row[at].wrapping_add(predicted);
    }
    Ok(())
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = i16::from(a) + i16::from(b) - i16::from(c);
    let (pa, pb, pc) = ((p - i16::from(a)).abs(), (p - i16::from(b)).abs(), (p - i16::from(c)).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// Whether a pixel is a mark: not white, and not fully transparent.
fn is_mark(pixel: &[u8], colour: u8, palette: &[[u8; 3]]) -> bool {
    match colour {
        0 => pixel[0] != 0xFF,
        4 => pixel[1] != 0 && pixel[0] != 0xFF,
        2 => pixel != [0xFF, 0xFF, 0xFF],
        6 => pixel[3] != 0 && pixel[..3] != [0xFF, 0xFF, 0xFF],
        _ => palette.get(usize::from(pixel[0])).is_some_and(|rgb| *rgb != [0xFF, 0xFF, 0xFF]),
    }
}
