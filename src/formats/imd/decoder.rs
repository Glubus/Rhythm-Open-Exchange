#![warn(clippy::pedantic)]
//! `.imd` decoder: the header, the BPM table and the 11-byte note records to a 4- to 6-column ROX chart. See `mod.rs` for the decisions.

use std::collections::HashSet;
use std::path::Path;

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, Note, RoxChart, TimingPoint};

/// Decoder of `.imd` files (Rhythm Master classic mode): one chart per file.
pub struct ImdDecoder;
crate::impl_format!(ImdDecoder, ["imd"]);

/// Files above this size are refused (a real one is a few tens of kilobytes).
pub(super) const MAX_FILE_SIZE: usize = 64 * 1024 * 1024;
/// Bytes of the header: total time (`u32`) and BPM entry count (`u32`).
pub(super) const HEADER_BYTES: usize = 8;
/// Bytes of a BPM entry: time in ms (`u32`) and BPM (`f64`).
pub(super) const BPM_ENTRY_BYTES: usize = 12;
/// Bytes of the separator and the note count that follow the BPM table.
pub(super) const NOTES_HEADER_BYTES: usize = 6;
/// The two bytes between the BPM table and the note count.
pub(super) const SEPARATOR: u16 = 0x0303;
/// Bytes of a note record: type (`u16`), time in ms (`u32`), track (`u8`), action (`i32`).
pub(super) const NOTE_BYTES: usize = 11;
/// The game has at most 6 lanes: a note on a higher track is not on the stage.
pub const MAX_LANES: u8 = 6;
/// Fewest columns of a chart (the game's smallest mode).
pub const MIN_LANES: u8 = 4;
/// How far apart, in ms, a hold's end and a slide of its chain may be: real files round a hold's length by one ms or two.
const LINK_TOLERANCE_MS: i64 = 2;
/// Largest BPM read (a BPM is a `f32` in the model).
const MAX_BPM: f64 = 10_000_000.0;

/// The difficulty a file name gives (`_ez`, `_nm`, `_hd`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImdDifficulty {
    /// `_ez`.
    Easy,
    /// `_nm`.
    Normal,
    /// `_hd`.
    Hard,
}

impl ImdDifficulty {
    /// The name written in `metadata.difficulty_name`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Easy => "Easy",
            Self::Normal => "Normal",
            Self::Hard => "Hard",
        }
    }
}

/// What a file name `<song>_<4|5|6>k_<ez|nm|hd>.imd` says: the song, the number of lanes and the difficulty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImdName<'a> {
    /// The file name without its extension and without the `_<n>k_<xx>` suffix: the part the song's audio and cover are named after
    /// (`<song>.mp3`). The whole stem when the name has no such suffix.
    pub song: &'a str,
    /// The lanes the suffix gives (4, 5 or 6).
    pub key_count: Option<u8>,
    pub difficulty: Option<ImdDifficulty>,
}

/// Splits the file name of an `.imd` (a path is cut at its last component). The suffix is `_4k_ez`, `_5k_nm`, `_6k_hd`... in any case
/// (3 lane counts x 3 difficulties); a name that does not end like that has no lane count and no difficulty, and its song is its whole stem.
#[must_use]
pub fn parse_name(file_name: &str) -> ImdName<'_> {
    let name = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    let stem = name
        .rsplit_once('.')
        .filter(|(_, extension)| extension.eq_ignore_ascii_case("imd"))
        .map_or(name, |(stem, _)| stem);
    let whole = ImdName {
        song: stem,
        key_count: None,
        difficulty: None,
    };
    // `_4k_hd` is 6 ASCII bytes: look at the tail as bytes so a multi-byte character before it is never cut.
    let bytes = stem.as_bytes();
    let Some(tail) = bytes.len().checked_sub(6).map(|at| &bytes[at..]) else {
        return whole;
    };
    let key_count = match tail {
        [b'_', digit @ b'4'..=b'6', b'k' | b'K', b'_', ..] => digit - b'0',
        _ => return whole,
    };
    let difficulty = match tail[4..].to_ascii_lowercase().as_slice() {
        b"ez" => ImdDifficulty::Easy,
        b"nm" => ImdDifficulty::Normal,
        b"hd" => ImdDifficulty::Hard,
        _ => return whole,
    };
    ImdName {
        // A name that is only the suffix keeps its whole stem: a chart is never titled with nothing.
        song: if bytes.len() == 6 {
            stem
        } else {
            &stem[..stem.len() - 6]
        },
        key_count: Some(key_count),
        difficulty: Some(difficulty),
    }
}

/// What the decoder dropped, simplified or repaired in a file. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // two flags, the rest are counters
pub struct ImdReport {
    /// Columns of the chart.
    pub key_count: u8,
    /// The columns come from the file name (`_4k_`, `_5k_`, `_6k_`); otherwise from the notes (at least 4, at most 6).
    pub key_count_from_name: bool,
    /// Total time of the song written in the header, in ms (not used: the chart has no duration).
    pub declared_duration_ms: u32,
    /// Entries of the BPM table.
    pub bpm_entries: usize,
    /// Timing points of the chart: the table's entries where the BPM changes (a table of one entry per beat at one tempo is one point).
    pub bpm_points: usize,
    /// Entries whose BPM is not a finite number in `(0, 1e7]`: ignored.
    pub invalid_bpm_entries: usize,
    /// The first usable entry came after the first note: its point was moved to the first note's time (the model needs a BPM by then).
    pub first_bpm_moved: bool,

    /// Single slides (`0x01`, a flick toward another lane): kept as taps on their own lane; the target lane is lost.
    pub single_slides_as_taps: usize,
    /// Hold segments of a hold-and-slide chain (`0x22`, `0x62`, `0xA2`): each one is a hold on its own lane; the link between them is lost.
    pub chain_holds: usize,
    /// Slides of a chain (`0x21`, `0x61`, `0xA1`) that sit at the end or at the start of a hold segment: the finger moves on, nothing is
    /// pressed again, so they are dropped.
    pub chain_joints_dropped: usize,
    /// Slides of a chain (`0x21`, `0x61`, `0xA1`) with no hold segment beside them: kept as taps rather than lost.
    pub lone_chain_slides_as_taps: usize,
    /// Holds with a length of 0 ms or less: kept as taps.
    pub empty_holds_as_taps: usize,

    /// Notes on a lane the chart has not (track 6 and above, or a lane the file name's lane count does not have): dropped.
    pub notes_outside_lanes: usize,
    /// Records with a type the game's charts do not use: dropped.
    pub unknown_types: usize,
    /// Two taps on the same lane and time: the first stays.
    pub duplicate_taps: usize,
    /// Notes that start inside a hold of their lane: dropped.
    pub overlapping_notes_dropped: usize,
}

/// A record of the file.
#[derive(Clone, Copy)]
struct Record {
    kind: Kind,
    time_ms: u32,
    track: u8,
    action: i32,
}

/// What a note type is, once the flag bits are read as the format notes give them.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// `0x00`.
    Tap,
    /// `0x01`.
    SingleSlide,
    /// `0x02`.
    Hold,
    /// `0x22`, `0x62`, `0xA2`.
    ChainHold,
    /// `0x21`, `0x61`, `0xA1`.
    ChainSlide,
}

fn kind_of(code: u16) -> Option<Kind> {
    Some(match code {
        0x00 => Kind::Tap,
        0x01 => Kind::SingleSlide,
        0x02 => Kind::Hold,
        0x22 | 0x62 | 0xA2 => Kind::ChainHold,
        0x21 | 0x61 | 0xA1 => Kind::ChainSlide,
        _ => return None,
    })
}

/// A note with its times in ms, before the overlaps are settled.
struct Timed {
    time_ms: i64,
    duration_ms: Option<i64>,
    column: u8,
}

/// The sizes of a file, checked against its length.
pub(super) struct Layout {
    pub bpm_count: usize,
    pub note_count: usize,
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    data.get(at..at.checked_add(4)?)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
}

/// Reads the counts of the header and checks the file is exactly `8 + 12n + 6 + 11m` bytes with the separator `0x0303` where it belongs.
fn layout(data: &[u8]) -> RoxResult<Layout> {
    let bad = |what: &str| RoxError::InvalidFormat(format!("imd: {what}"));
    if data.len() > MAX_FILE_SIZE {
        return Err(bad("file too large"));
    }
    let bpm_count = u32_at(data, 4).ok_or_else(|| bad("truncated header"))?;
    let table_end = HEADER_BYTES as u64 + BPM_ENTRY_BYTES as u64 * u64::from(bpm_count);
    let notes_start = table_end + NOTES_HEADER_BYTES as u64;
    if notes_start > data.len() as u64 {
        return Err(bad("the BPM table runs past the end of the file"));
    }
    // `notes_start <= data.len()`, so every offset below fits in a usize.
    let table_end = usize::try_from(table_end).map_err(|_| bad("file too large"))?;
    let separator = data
        .get(table_end..table_end + 2)
        .ok_or_else(|| bad("truncated"))?;
    if u16::from_le_bytes([separator[0], separator[1]]) != SEPARATOR {
        return Err(bad("the separator 0x0303 is not after the BPM table"));
    }
    let note_count = u32_at(data, table_end + 2).ok_or_else(|| bad("truncated"))?;
    let expected = notes_start + NOTE_BYTES as u64 * u64::from(note_count);
    if expected != data.len() as u64 {
        return Err(bad(
            "the size of the file is not 8 + 12 x BPM entries + 6 + 11 x notes",
        ));
    }
    if bpm_count == 0 {
        return Err(bad("no BPM entry"));
    }
    Ok(Layout {
        bpm_count: bpm_count as usize,
        note_count: note_count as usize,
    })
}

impl Decoder for ImdDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode(data, "", None).map(|(chart, _)| chart)
    }
}

impl ImdDecoder {
    /// Decodes an `.imd` with the given title; the lane count comes from the notes and there is no difficulty name.
    ///
    /// # Errors
    /// Returns an error if the file is not exactly an `.imd` (size, separator), has no usable BPM or no playable note.
    pub fn decode_with_title(data: &[u8], title: &str) -> RoxResult<RoxChart> {
        decode(data, "", Some(title)).map(|(chart, _)| chart)
    }

    /// Decodes an `.imd` from its file name (a path is cut at its last component): the lane count (`_4k_`, `_5k_`, `_6k_`) and the
    /// difficulty (`_ez`, `_nm`, `_hd`) come from the name when it has them, the title is the part before them (see [`parse_name`]).
    ///
    /// # Errors
    /// Returns an error if the file is not exactly an `.imd` (size, separator), has no usable BPM or no playable note.
    pub fn decode_with_name(data: &[u8], file_name: &str) -> RoxResult<RoxChart> {
        decode(data, file_name, None).map(|(chart, _)| chart)
    }

    /// Decodes an `.imd` as [`ImdDecoder::decode_with_name`] does (`file_name` is empty when there is none) and says what was dropped or simplified.
    ///
    /// # Errors
    /// Returns an error if the file is not exactly an `.imd` (size, separator), has no usable BPM or no playable note.
    pub fn decode_with_report(data: &[u8], file_name: &str) -> RoxResult<(RoxChart, ImdReport)> {
        decode(data, file_name, None)
    }

    /// Decodes the `.imd` at `path`, named as [`ImdDecoder::decode_with_name`] does.
    ///
    /// # Errors
    /// Returns an error if the file can not be read, is too large, or does not decode.
    pub fn decode_file(path: &Path) -> RoxResult<RoxChart> {
        if std::fs::metadata(path)?.len() > MAX_FILE_SIZE as u64 {
            return Err(RoxError::InvalidFormat("imd: file too large".to_string()));
        }
        let name = path.file_name().map(|name| name.to_string_lossy());
        Self::decode_with_name(&std::fs::read(path)?, name.as_deref().unwrap_or(""))
    }
}

fn decode(data: &[u8], file_name: &str, title: Option<&str>) -> RoxResult<(RoxChart, ImdReport)> {
    let layout = layout(data)?;
    let name = parse_name(file_name);
    let mut report = ImdReport {
        declared_duration_ms: u32_at(data, 0).unwrap_or(0),
        bpm_entries: layout.bpm_count,
        ..ImdReport::default()
    };

    let notes_start = HEADER_BYTES + BPM_ENTRY_BYTES * layout.bpm_count + NOTES_HEADER_BYTES;
    let mut records = Vec::with_capacity(layout.note_count);
    for bytes in data[notes_start..].as_chunks::<NOTE_BYTES>().0 {
        let code = u16::from_le_bytes([bytes[0], bytes[1]]);
        let Some(kind) = kind_of(code) else {
            report.unknown_types += 1;
            continue;
        };
        records.push(Record {
            kind,
            time_ms: u32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]),
            track: bytes[6],
            action: i32::from_le_bytes([bytes[7], bytes[8], bytes[9], bytes[10]]),
        });
    }

    let key_count = name.key_count.unwrap_or_else(|| {
        let highest = records
            .iter()
            .filter(|record| record.track < MAX_LANES)
            .map(|record| record.track + 1)
            .max()
            .unwrap_or(0);
        highest.clamp(MIN_LANES, MAX_LANES)
    });
    report.key_count = key_count;
    report.key_count_from_name = name.key_count.is_some();

    let timed = place_notes(&records, key_count, &mut report);
    let notes = settle(timed, &mut report);
    if notes.is_empty() {
        return Err(RoxError::InvalidFormat("imd: no playable note".to_string()));
    }

    let mut chart = RoxChart::new(key_count);
    let file_title = title.unwrap_or(name.song);
    chart.metadata = Metadata {
        title: file_title.into(),
        difficulty_name: name.difficulty.map_or("", ImdDifficulty::name).into(),
        ..Metadata::default()
    };
    let first_note = notes[0].time_us;
    chart.notes = notes;
    chart.timing_points = timing_points(data, &layout, first_note, &mut report)?;
    chart.validate()?;
    Ok((chart, report))
}

/// What the notes become: taps and holds with their lane, the chain slides settled against the hold segments around them.
fn place_notes(records: &[Record], key_count: u8, report: &mut ImdReport) -> Vec<Timed> {
    // Where each hold segment starts and ends, `(ms, track)`: the slides of a chain sit on those points.
    let mut starts: HashSet<(i64, i64)> = HashSet::new();
    let mut ends: HashSet<(i64, i64)> = HashSet::new();
    for record in records {
        if matches!(record.kind, Kind::Hold | Kind::ChainHold) && record.action > 0 {
            let start = i64::from(record.time_ms);
            starts.insert((start, i64::from(record.track)));
            ends.insert((start + i64::from(record.action), i64::from(record.track)));
        }
    }
    let near = |set: &HashSet<(i64, i64)>, time: i64, track: i64| {
        (-LINK_TOLERANCE_MS..=LINK_TOLERANCE_MS).any(|shift| set.contains(&(time + shift, track)))
    };

    let mut timed = Vec::with_capacity(records.len());
    for record in records {
        if record.track >= key_count {
            report.notes_outside_lanes += 1;
            continue;
        }
        let time_ms = i64::from(record.time_ms);
        let tap = |column| Timed {
            time_ms,
            duration_ms: None,
            column,
        };
        match record.kind {
            Kind::Tap => timed.push(tap(record.track)),
            Kind::SingleSlide => {
                report.single_slides_as_taps += 1;
                timed.push(tap(record.track));
            }
            Kind::Hold | Kind::ChainHold => {
                if record.kind == Kind::ChainHold {
                    report.chain_holds += 1;
                }
                if record.action > 0 {
                    timed.push(Timed {
                        time_ms,
                        duration_ms: Some(i64::from(record.action)),
                        column: record.track,
                    });
                } else {
                    report.empty_holds_as_taps += 1;
                    timed.push(tap(record.track));
                }
            }
            Kind::ChainSlide => {
                let track = i64::from(record.track);
                let target = track + i64::from(record.action);
                if near(&ends, time_ms, track) || near(&starts, time_ms, target) {
                    report.chain_joints_dropped += 1;
                } else {
                    report.lone_chain_slides_as_taps += 1;
                    timed.push(tap(record.track));
                }
            }
        }
    }
    timed
}

/// Sorts the notes by time and lane (holds first at a tie) and drops the ones the model forbids: inside a hold of their lane, and a
/// second tap on the same lane and time. Times become microseconds.
fn settle(mut timed: Vec<Timed>, report: &mut ImdReport) -> Vec<Note> {
    timed.sort_by(|a, b| {
        a.time_ms
            .cmp(&b.time_ms)
            .then(a.column.cmp(&b.column))
            .then(b.duration_ms.is_some().cmp(&a.duration_ms.is_some()))
    });
    let mut busy_until = [i64::MIN; MAX_LANES as usize];
    let mut last_tap: [Option<i64>; MAX_LANES as usize] = [None; MAX_LANES as usize];
    let mut kept = Vec::with_capacity(timed.len());
    for note in timed {
        let lane = usize::from(note.column);
        if note.time_ms < busy_until[lane] {
            report.overlapping_notes_dropped += 1;
            continue;
        }
        if note.duration_ms.is_none() && last_tap[lane] == Some(note.time_ms) {
            report.duplicate_taps += 1;
            continue;
        }
        if let Some(duration) = note.duration_ms {
            busy_until[lane] = note.time_ms + duration;
            last_tap[lane] = None;
            kept.push(Note::hold(
                note.time_ms * 1000,
                duration * 1000,
                note.column,
            ));
        } else {
            last_tap[lane] = Some(note.time_ms);
            kept.push(Note::tap(note.time_ms * 1000, note.column));
        }
    }
    kept
}

/// The BPM table as timing points: the usable entries sorted by time (of two at the same time the last wins), one point where the BPM
/// changes, the first no later than the first note.
fn timing_points(
    data: &[u8],
    layout: &Layout,
    first_note_us: i64,
    report: &mut ImdReport,
) -> RoxResult<Vec<TimingPoint>> {
    let mut entries: Vec<(u32, f64)> = Vec::with_capacity(layout.bpm_count);
    for bytes in data[HEADER_BYTES..]
        .as_chunks::<BPM_ENTRY_BYTES>()
        .0
        .iter()
        .take(layout.bpm_count)
    {
        let time_ms = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let bpm = f64::from_le_bytes([
            bytes[4], bytes[5], bytes[6], bytes[7], bytes[8], bytes[9], bytes[10], bytes[11],
        ]);
        if bpm.is_finite() && bpm > 0.0 && bpm <= MAX_BPM {
            entries.push((time_ms, bpm));
        } else {
            report.invalid_bpm_entries += 1;
        }
    }
    entries.sort_by_key(|entry| entry.0);
    let mut points: Vec<(i64, f32)> = Vec::new();
    for (index, &(time_ms, bpm)) in entries.iter().enumerate() {
        if entries.get(index + 1).is_some_and(|next| next.0 == time_ms) {
            continue;
        }
        #[allow(clippy::cast_possible_truncation)] // `bpm` is in (0, 1e7]
        let bpm = bpm as f32;
        if points
            .last()
            .is_none_or(|last| last.1.to_bits() != bpm.to_bits())
        {
            points.push((i64::from(time_ms) * 1000, bpm));
        }
    }
    let Some(first) = points.first_mut() else {
        return Err(RoxError::InvalidFormat("imd: no usable BPM".to_string()));
    };
    if first.0 > first_note_us {
        first.0 = first_note_us;
        report.first_bpm_moved = true;
    }
    report.bpm_points = points.len();
    Ok(points
        .into_iter()
        .map(|(time_us, bpm)| TimingPoint::Bpm {
            time_us,
            bpm,
            signature: 4,
        })
        .collect())
}
