#![warn(clippy::pedantic)]
//! The `.ojm` sample container of `O2Jam`: M30, OMC and OJM.
//!
//! Layouts and scrambles are the ones of `vsrg-format-docs/vsrg/o2jam/ojm` in Prism's repository (itself built from the
//! `Open2Jam` parser and the CXO2 documentation). Nothing here decodes audio: a sample is handed over as the bytes of
//! an Ogg Vorbis file or of a complete RIFF/WAVE file.
//!
//! - **M30**: 28-byte header, then `52`-byte sample headers each followed by an Ogg stream. `encryption_flag` 0 is plain,
//!   16 XORs every complete group of 4 bytes with `nami`, 32 with `0412`; any other flag is an error (the flags 1, 2, 4
//!   and 8 are only named in the `Open2Jam` comments, nobody documents what they do). Codec 5 is a key sound (id =
//!   `ref`), codec 0 a background sound (id = `1000 + ref`), any other codec is skipped and counted.
//! - **OMC** / **OJM**: 20-byte header, a WAV section of 56-byte headers + headerless PCM (ids from 0, an empty slot still
//!   takes an id), then an Ogg section of 36-byte headers (ids from 1000, same rule). OMC scrambles the PCM (block
//!   rearrangement then the state-carrying "acc xor"), OJM leaves it plain. The PCM is returned inside a 44-byte RIFF
//!   header built from the sample header, so `data` of a WAV sample is a file any audio reader opens.
//!
//! Input is untrusted: every size is checked against the bytes that remain before anything is allocated, a section that
//! runs past the end of the file stops the parse (`truncated`), it never panics.

use std::collections::BTreeMap;

use rox::error::{RoxError, RoxResult};

/// `M30\0`.
const M30_SIGNATURE: [u8; 4] = *b"M30\0";
/// `OMC\0`.
const OMC_SIGNATURE: [u8; 4] = *b"OMC\0";
/// `OJM\0`.
const OJM_SIGNATURE: [u8; 4] = *b"OJM\0";

const M30_HEADER: usize = 28;
const M30_SAMPLE_HEADER: usize = 52;
const OMC_HEADER: usize = 20;
const OMC_WAV_HEADER: usize = 56;
const OMC_OGG_HEADER: usize = 36;
/// First id of the Ogg samples (OMC/OJM Ogg section, M30 background sounds).
const OGG_BASE: u32 = 1000;
const MASK_NAMI: [u8; 4] = *b"nami";
const MASK_0412: [u8; 4] = *b"0412";

/// Block rearrangement table of the OMC scramble (290 entries, values 0..=16). The `Open2Jam` parser and the CXO2
/// client carry the same table (compared entry by entry): it is a constant of the format, a dump of the original tool.
pub(super) const REARRANGE_TABLE: [u8; 290] = [
    0x10, 0x0E, 0x02, 0x09, 0x04, 0x00, 0x07, 0x01, 0x06, 0x08, 0x0F, 0x0A, 0x05, 0x0C, 0x03, 0x0D,
    0x0B, 0x07, 0x02, 0x0A, 0x0B, 0x03, 0x05, 0x0D, 0x08, 0x04, 0x00, 0x0C, 0x06, 0x0F, 0x0E, 0x10,
    0x01, 0x09, 0x0C, 0x0D, 0x03, 0x00, 0x06, 0x09, 0x0A, 0x01, 0x07, 0x08, 0x10, 0x02, 0x0B, 0x0E,
    0x04, 0x0F, 0x05, 0x08, 0x03, 0x04, 0x0D, 0x06, 0x05, 0x0B, 0x10, 0x02, 0x0C, 0x07, 0x09, 0x0A,
    0x0F, 0x0E, 0x00, 0x01, 0x0F, 0x02, 0x0C, 0x0D, 0x00, 0x04, 0x01, 0x05, 0x07, 0x03, 0x09, 0x10,
    0x06, 0x0B, 0x0A, 0x08, 0x0E, 0x00, 0x04, 0x0B, 0x10, 0x0F, 0x0D, 0x0C, 0x06, 0x05, 0x07, 0x01,
    0x02, 0x03, 0x08, 0x09, 0x0A, 0x0E, 0x03, 0x10, 0x08, 0x07, 0x06, 0x09, 0x0E, 0x0D, 0x00, 0x0A,
    0x0B, 0x04, 0x05, 0x0C, 0x02, 0x01, 0x0F, 0x04, 0x0E, 0x10, 0x0F, 0x05, 0x08, 0x07, 0x0B, 0x00,
    0x01, 0x06, 0x02, 0x0C, 0x09, 0x03, 0x0A, 0x0D, 0x06, 0x0D, 0x0E, 0x07, 0x10, 0x0A, 0x0B, 0x00,
    0x01, 0x0C, 0x0F, 0x02, 0x03, 0x08, 0x09, 0x04, 0x05, 0x0A, 0x0C, 0x00, 0x08, 0x09, 0x0D, 0x03,
    0x04, 0x05, 0x10, 0x0E, 0x0F, 0x01, 0x02, 0x0B, 0x06, 0x07, 0x05, 0x06, 0x0C, 0x04, 0x0D, 0x0F,
    0x07, 0x0E, 0x08, 0x01, 0x09, 0x02, 0x10, 0x0A, 0x0B, 0x00, 0x03, 0x0B, 0x0F, 0x04, 0x0E, 0x03,
    0x01, 0x00, 0x02, 0x0D, 0x0C, 0x06, 0x07, 0x05, 0x10, 0x09, 0x08, 0x0A, 0x03, 0x02, 0x01, 0x00,
    0x04, 0x0C, 0x0D, 0x0B, 0x10, 0x05, 0x06, 0x0F, 0x0E, 0x07, 0x09, 0x0A, 0x08, 0x09, 0x0A, 0x00,
    0x07, 0x08, 0x06, 0x10, 0x03, 0x04, 0x01, 0x02, 0x05, 0x0B, 0x0E, 0x0F, 0x0D, 0x0C, 0x0A, 0x06,
    0x09, 0x0C, 0x0B, 0x10, 0x07, 0x08, 0x00, 0x0F, 0x03, 0x01, 0x02, 0x05, 0x0D, 0x0E, 0x04, 0x0D,
    0x00, 0x01, 0x0E, 0x02, 0x03, 0x08, 0x0B, 0x07, 0x0C, 0x09, 0x05, 0x0A, 0x0F, 0x04, 0x06, 0x10,
    0x01, 0x0E, 0x02, 0x03, 0x0D, 0x0B, 0x07, 0x00, 0x08, 0x0C, 0x09, 0x06, 0x0F, 0x10, 0x05, 0x0A,
    0x04, 0x00,
];

/// Which of the three container layouts a file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OjmKind {
    M30,
    Omc,
    Ojm,
}

/// What a sample's `data` holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleFormat {
    /// A complete RIFF/WAVE file (the PCM of the container behind a rebuilt 44-byte header).
    Wav,
    /// An Ogg Vorbis stream.
    Ogg,
}

/// One sample of the container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OjmSample {
    /// The id a `.ojn` note refers to it by: `value - 1` of the note, plus 1000 when the note type has bit 2 set.
    /// Wav samples are `0..1000` and Ogg samples `1000..` in OMC/OJM; in M30 key sounds (codec 5) are `ref` and
    /// background sounds (codec 0) `1000 + ref`.
    pub id: u32,
    pub format: SampleFormat,
    /// Name stored in the container (informational; may be empty or not meaningful).
    pub name: String,
    /// The bytes of an Ogg file or of a WAV file, see [`SampleFormat`].
    pub data: Vec<u8>,
}

/// What the parser skipped or could not finish.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OjmReport {
    /// A header or a payload ran past the end of the file: the samples after it are not read.
    pub truncated: bool,
    /// M30 samples with a codec other than 0 and 5: skipped.
    pub unknown_codec_samples: usize,
    /// Samples whose id was already taken: the first one is kept.
    pub duplicate_ids: usize,
}

/// A parsed `.ojm`: the samples by id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OjmContainer {
    kind: OjmKind,
    samples: BTreeMap<u32, OjmSample>,
    report: OjmReport,
}

impl OjmContainer {
    /// Parses the bytes of an `.ojm` file (M30, OMC or OJM, found from the signature).
    ///
    /// # Errors
    /// Returns an error if the signature is none of the three, the header is cut short, or an M30 file uses an
    /// encryption flag other than 0, 16 and 32. A file that is cut inside its samples is NOT an error: the samples
    /// read so far are returned and [`OjmReport::truncated`] is set.
    pub fn parse(data: &[u8]) -> RoxResult<Self> {
        let signature: [u8; 4] =
            data.get(..4)
                .and_then(|s| s.try_into().ok())
                .ok_or_else(|| {
                    RoxError::InvalidFormat("ojm: file shorter than its signature".into())
                })?;
        let mut report = OjmReport::default();
        let (kind, samples) = match signature {
            M30_SIGNATURE => (OjmKind::M30, parse_m30(data, &mut report)?),
            OMC_SIGNATURE => (OjmKind::Omc, parse_omc(data, true, &mut report)?),
            OJM_SIGNATURE => (OjmKind::Ojm, parse_omc(data, false, &mut report)?),
            other => {
                return Err(RoxError::InvalidFormat(format!(
                    "ojm: unknown signature {other:02X?}"
                )));
            }
        };
        Ok(Self {
            kind,
            samples,
            report,
        })
    }

    #[must_use]
    pub fn kind(&self) -> OjmKind {
        self.kind
    }

    /// What the parser skipped or could not finish.
    #[must_use]
    pub fn report(&self) -> OjmReport {
        self.report
    }

    /// The samples by increasing id.
    pub fn samples(&self) -> impl Iterator<Item = &OjmSample> {
        self.samples.values()
    }

    /// The sample with this id, as an `.ojn` note references it.
    #[must_use]
    pub fn sample(&self, id: u32) -> Option<&OjmSample> {
        self.samples.get(&id)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

/// Little-endian reads at an offset; `None` past the end.
fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        data.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn i32_at(data: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(
        data.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn name_of(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn insert(samples: &mut BTreeMap<u32, OjmSample>, sample: OjmSample, report: &mut OjmReport) {
    match samples.entry(sample.id) {
        std::collections::btree_map::Entry::Vacant(slot) => {
            slot.insert(sample);
        }
        std::collections::btree_map::Entry::Occupied(_) => report.duplicate_ids += 1,
    }
}

fn parse_m30(data: &[u8], report: &mut OjmReport) -> RoxResult<BTreeMap<u32, OjmSample>> {
    let flag = i32_at(data, 8)
        .filter(|_| data.len() >= M30_HEADER)
        .ok_or_else(|| RoxError::InvalidFormat("m30: header is cut short".into()))?;
    let mask = match flag {
        0 => None,
        16 => Some(MASK_NAMI),
        32 => Some(MASK_0412),
        other => {
            return Err(RoxError::InvalidFormat(format!(
                "m30: encryption flag {other} is not supported (0, 16 and 32 are)"
            )));
        }
    };
    let mut samples = BTreeMap::new();
    let mut at = M30_HEADER;
    // The sample count of the header is not trusted (the format notes say it differs from the real number): read
    // until fewer than a sample header remains.
    while data.len() - at >= M30_SAMPLE_HEADER {
        let header = &data[at..at + M30_SAMPLE_HEADER];
        let size = i32_at(header, 32).and_then(|s| usize::try_from(s).ok());
        let Some(size) = size.filter(|&s| s <= data.len() - at - M30_SAMPLE_HEADER) else {
            report.truncated = true;
            break;
        };
        let codec = u16_at(header, 36).unwrap_or(u16::MAX);
        let reference = u32::from(u16_at(header, 44).unwrap_or(0));
        let payload = &data[at + M30_SAMPLE_HEADER..at + M30_SAMPLE_HEADER + size];
        at += M30_SAMPLE_HEADER + size;
        let id = match codec {
            5 => reference,
            0 => OGG_BASE + reference,
            _ => {
                report.unknown_codec_samples += 1;
                continue;
            }
        };
        if size == 0 {
            continue;
        }
        let mut bytes = payload.to_vec();
        if let Some(mask) = mask {
            xor_groups(&mut bytes, mask);
        }
        insert(
            &mut samples,
            OjmSample {
                id,
                format: SampleFormat::Ogg,
                name: name_of(&header[..32]),
                data: bytes,
            },
            report,
        );
    }
    Ok(samples)
}

/// XOR with `mask` every complete group of four bytes; the trailing 1..3 bytes stay as they are.
fn xor_groups(bytes: &mut [u8], mask: [u8; 4]) {
    let (groups, _) = bytes.as_chunks_mut::<4>();
    for group in groups {
        for (byte, key) in group.iter_mut().zip(mask) {
            *byte ^= key;
        }
    }
}

fn parse_omc(
    data: &[u8],
    scrambled: bool,
    report: &mut OjmReport,
) -> RoxResult<BTreeMap<u32, OjmSample>> {
    if data.len() < OMC_HEADER {
        return Err(RoxError::InvalidFormat(
            "omc/ojm: header is cut short".into(),
        ));
    }
    let ogg_start = i32_at(data, 12)
        .and_then(|v| usize::try_from(v).ok())
        .unwrap_or(0);
    let file_size = i32_at(data, 16)
        .and_then(|v| usize::try_from(v).ok())
        .map_or(data.len(), |size| size.min(data.len()));
    let mut samples = BTreeMap::new();
    let mut at = OMC_HEADER;
    let mut id = 0u32;
    let mut acc = AccXor::new();
    // The sections are walked like the `Open2Jam` parser does: the WAV section runs while the offset is before
    // `ogg_start` (the counts of the header are not used), the Ogg section from where it stopped to the file size.
    while at < ogg_start {
        if file_size.saturating_sub(at) < OMC_WAV_HEADER {
            report.truncated = true;
            return Ok(samples);
        }
        let header = &data[at..at + OMC_WAV_HEADER];
        let size = i32_at(header, 52).and_then(|s| usize::try_from(s).ok());
        let Some(size) = size.filter(|&s| s <= file_size - at - OMC_WAV_HEADER) else {
            report.truncated = true;
            return Ok(samples);
        };
        at += OMC_WAV_HEADER + size;
        if size == 0 {
            id += 1;
            continue;
        }
        let mut pcm = data[at - size..at].to_vec();
        if scrambled {
            pcm = rearrange(&pcm);
            acc.decode(&mut pcm);
        }
        let fmt = WavFormat {
            audio_format: u16_at(header, 32).unwrap_or(0),
            channels: u16_at(header, 34).unwrap_or(0),
            sample_rate: i32_at(header, 36).unwrap_or(0).cast_unsigned(),
            byte_rate: i32_at(header, 40).unwrap_or(0).cast_unsigned(),
            block_align: u16_at(header, 44).unwrap_or(0),
            bits_per_sample: u16_at(header, 46).unwrap_or(0),
        };
        insert(
            &mut samples,
            OjmSample {
                id,
                format: SampleFormat::Wav,
                name: name_of(&header[..32]),
                data: wav_file(fmt, &pcm),
            },
            report,
        );
        id += 1;
    }
    let mut id = OGG_BASE;
    while at < file_size {
        if file_size - at < OMC_OGG_HEADER {
            report.truncated = true;
            break;
        }
        let header = &data[at..at + OMC_OGG_HEADER];
        let size = i32_at(header, 32).and_then(|s| usize::try_from(s).ok());
        let Some(size) = size.filter(|&s| s <= file_size - at - OMC_OGG_HEADER) else {
            report.truncated = true;
            break;
        };
        at += OMC_OGG_HEADER + size;
        if size > 0 {
            insert(
                &mut samples,
                OjmSample {
                    id,
                    format: SampleFormat::Ogg,
                    name: name_of(&header[..32]),
                    data: data[at - size..at].to_vec(),
                },
                report,
            );
        }
        id += 1;
    }
    Ok(samples)
}

/// Undoes the block shuffle of one OMC chunk: the chunk is cut in 17 blocks of `len / 17` bytes, block `b` of the
/// file goes to the place `REARRANGE_TABLE[key + b]` with `key = 17 * (len % 17)`; the `len % 17` bytes after the 17
/// blocks stay where they are.
pub(super) fn rearrange(encoded: &[u8]) -> Vec<u8> {
    let len = encoded.len();
    let block = len / 17;
    let key = 17 * (len % 17);
    let mut plain = encoded.to_vec();
    for b in 0..17 {
        let to = block * usize::from(REARRANGE_TABLE[key + b]);
        plain[to..to + block].copy_from_slice(&encoded[block * b..block * (b + 1)]);
    }
    plain
}

/// The "acc xor" of OMC: a byte is complemented when bit 7 of `key << counter` is set; `key` is the last byte (as it was
/// before the complement) of the previous group of 8, so the state runs across chunks of one file.
pub(super) struct AccXor {
    pub(super) key: u8,
    pub(super) counter: u8,
}

impl AccXor {
    pub(super) fn new() -> Self {
        Self {
            key: 0xFF,
            counter: 0,
        }
    }

    pub(super) fn decode(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            let before = *byte;
            if (self.key << self.counter) & 0x80 != 0 {
                *byte = !*byte;
            }
            self.counter += 1;
            if self.counter > 7 {
                self.counter = 0;
                self.key = before;
            }
        }
    }
}

/// The fields of an OMC/OJM WAV sample header that make a `fmt ` chunk.
#[derive(Debug, Clone, Copy)]
struct WavFormat {
    audio_format: u16,
    channels: u16,
    sample_rate: u32,
    byte_rate: u32,
    block_align: u16,
    bits_per_sample: u16,
}

/// A complete WAV file: `RIFF`, a 16-byte `fmt ` chunk from the sample header, then `pcm` as the `data` chunk (a zero
/// byte pads an odd length, as RIFF wants, without being counted in the chunk).
fn wav_file(fmt: WavFormat, pcm: &[u8]) -> Vec<u8> {
    let len = u32::try_from(pcm.len()).unwrap_or(u32::MAX);
    let pad = pcm.len() % 2;
    let mut out = Vec::with_capacity(44 + pcm.len() + pad);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&len.saturating_add(36 + u32::from(pad == 1)).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&fmt.audio_format.to_le_bytes());
    out.extend_from_slice(&fmt.channels.to_le_bytes());
    out.extend_from_slice(&fmt.sample_rate.to_le_bytes());
    out.extend_from_slice(&fmt.byte_rate.to_le_bytes());
    out.extend_from_slice(&fmt.block_align.to_le_bytes());
    out.extend_from_slice(&fmt.bits_per_sample.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(pcm);
    out.extend(std::iter::repeat_n(0u8, pad));
    out
}
