#![warn(clippy::pedantic)]
//! `.ssq` decoder: chunks to a four- or eight-column ROX chart per difficulty.
//!
//! See `mod.rs` for the decisions. The steps are: the chunk walk (`chunks.rs`), the tempo chunk (`tempo.rs`), then for each
//! step chunk the rows, the freeze block and the mines (here).

use std::collections::HashMap;
use std::path::Path;

use rox::codec::Decoder;
use rox::error::{RoxError, RoxResult};
use rox::model::{Metadata, Note, RoxChart, TimingPoint};

use super::chunks::{Chunk, i32_at, kind, walk};
use super::tempo::TempoMap;

/// Decoder of `.ssq` files: the first playable chart of the file (see [`SsqDecoder::variants`] for the others).
pub struct SsqDecoder;
crate::impl_format!(SsqDecoder, ["ssq"]);

/// Files above this size are refused (real ones are a few hundred kilobytes at most).
const MAX_FILE_SIZE: usize = 64 * 1024 * 1024;
/// Times further than this many seconds from the start of the audio are not read (a tiny tempo slope could send them out of range).
const MAX_SECONDS: i64 = 10_000_000;

/// The play style of a step chunk (low byte of its difficulty code).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Style {
    /// `0x14`: 4 panels, one player.
    Single,
    /// `0x18`: 8 panels, both sides.
    Double,
}

impl Style {
    /// Columns of a chart of this style.
    #[must_use]
    pub fn key_count(self) -> u8 {
        match self {
            Self::Single => 4,
            Self::Double => 8,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Single => "Single",
            Self::Double => "Double",
        }
    }
}

/// The difficulty slot of a step chunk (high byte of its difficulty code), named as the format notes name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Difficulty {
    /// Slot `0x04`.
    Beginner,
    /// Slot `0x01` (alternative names: Light).
    Basic,
    /// Slot `0x02` (Standard, Another, Trick).
    Difficult,
    /// Slot `0x03` (Heavy, Maniac, SSR).
    Expert,
    /// Slot `0x06` (Oni, CHAOS).
    Challenge,
}

impl Difficulty {
    /// The name written in `metadata.difficulty_name`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Beginner => "Beginner",
            Self::Basic => "Basic",
            Self::Difficult => "Difficult",
            Self::Expert => "Expert",
            Self::Challenge => "Challenge",
        }
    }
}

/// Splits a difficulty code (`param2` of a step chunk) into its style and slot; `None` for a code the game does not accept
/// (slot `0x05`, another style).
fn split_code(code: u16) -> Option<(Style, Difficulty)> {
    let style = match code & 0xFF {
        0x14 => Style::Single,
        0x18 => Style::Double,
        _ => return None,
    };
    let slot = match code >> 8 {
        0x01 => Difficulty::Basic,
        0x02 => Difficulty::Difficult,
        0x03 => Difficulty::Expert,
        0x04 => Difficulty::Beginner,
        0x06 => Difficulty::Challenge,
        _ => return None,
    };
    Some((style, slot))
}

/// A playable chart of an `.ssq` file. `index` counts the playable ones from 0, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    /// Number to give to [`SsqDecoder::decode_variant`]: `0..variants.len()`.
    pub index: usize,
    /// The difficulty code of the chunk (`0x0314`: slot 3, style `0x14`).
    pub code: u16,
    pub style: Style,
    pub difficulty: Difficulty,
    /// Notes of the decoded chart (a freeze is one, a shock arrow is one mine per panel).
    pub notes: usize,
    /// Freezes among them.
    pub holds: usize,
    /// Mines among them (shock arrows and the mine chunk).
    pub mines: usize,
    pub key_count: u8,
    pub duration_us: i64,
}

impl Variant {
    /// What identifies the chart in a message: `Single Expert`, `Double Challenge`.
    #[must_use]
    pub fn name(&self) -> String {
        format!("{} {}", self.style.name(), self.difficulty.name())
    }
}

/// What the decoder chose, skipped or repaired for a file and one of its charts. Nothing here changes the chart; it
/// explains it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SsqReport {
    /// Difficulty code of the chart.
    pub code: u16,
    /// Ticks per second of the tempo chunk.
    pub ticks_per_second: u16,
    /// Why the file's chunks were left out (unreadable, unsupported difficulty code, duplicate, no playable note, Hudson
    /// charts) or the walk stopped (bad length, past the end of the file, `param2` sentinel). The numbering of the playable
    /// charts is not affected by them.
    pub skipped: Vec<String>,
    /// Rows `0x0F`, `0xF0` or `0xFF` read as shock arrows (one mine per panel of the side).
    pub shock_arrows: usize,
    /// Rows with a panel the style does not have (a bit above the fourth in a single chart): the panel is dropped.
    pub panels_outside_style: usize,
    /// Rows with a negative tick: skipped.
    pub negative_ticks: usize,
    /// Freeze ends of kind other than `1`: ignored.
    pub ignored_freeze_kinds: usize,
    /// Panels of a freeze end with no earlier note on them (or one that is a shock arrow): ignored.
    pub freeze_ends_without_head: usize,
    /// Freezes that end at or before their head: kept as taps.
    pub empty_freezes_as_taps: usize,
    /// Entries of the mine chunk applied.
    pub mines_from_chunk: usize,
    /// Entries of the mine chunk refused (flags or reserved not 0, no panel, a shock pattern, a panel the style does not have, a negative tick).
    pub invalid_mine_entries: usize,
    /// Segments of the tempo chunk where the tick advances and the clock does not: no BPM.
    pub instant_tempo_segments: usize,
    /// Notes later than 10^7 seconds from the start of the audio: skipped.
    pub notes_out_of_range: usize,
    /// Notes inside a freeze of their column: dropped.
    pub overlapping_notes_dropped: usize,
}

/// A step chunk with its mines and the numbers read from its header.
struct ChartChunk<'a> {
    code: u16,
    style: Style,
    difficulty: Difficulty,
    chunk: Chunk<'a>,
    mines: Option<Chunk<'a>>,
}

/// A parsed file: the tempo and the playable chunks, no chart decoded yet.
struct Parsed<'a> {
    tempo: TempoMap,
    tps: u16,
    charts: Vec<ChartChunk<'a>>,
    skipped: Vec<String>,
}

/// A decoded chart and what is needed to list and explain it.
struct Built {
    chart: RoxChart,
    report: SsqReport,
    holds: usize,
    mines: usize,
    code: u16,
    style: Style,
    difficulty: Difficulty,
}

impl Decoder for SsqDecoder {
    fn decode_inner(data: &[u8]) -> RoxResult<RoxChart> {
        decode_nth(data, 0, "").map(|built| built.chart)
    }
}

impl SsqDecoder {
    /// The playable charts of the file, in file order: step chunks (type 3) with a difficulty code the game accepts, the first
    /// one of each code, with at least one playable note. The reasons for the others are in [`SsqReport::skipped`].
    ///
    /// # Errors
    /// Returns an error if the file has no readable tempo chunk (type 1).
    pub fn variants(data: &[u8]) -> RoxResult<Vec<Variant>> {
        let built = build_all(data, usize::MAX, "")?;
        Ok(built
            .iter()
            .enumerate()
            .map(|(index, one)| Variant {
                index,
                code: one.code,
                style: one.style,
                difficulty: one.difficulty,
                notes: one.chart.notes.len(),
                holds: one.holds,
                mines: one.mines,
                key_count: one.chart.key_count,
                duration_us: one.chart.duration_us(),
            })
            .collect())
    }

    /// Decodes playable chart `index` (the numbering of [`Self::variants`]). The title is empty: the format has none (see
    /// [`Self::decode_with_title`]); the audio file is empty too.
    ///
    /// # Errors
    /// Returns an error if the file is not an `.ssq` or has no such playable chart.
    pub fn decode_variant(data: &[u8], index: usize) -> RoxResult<RoxChart> {
        decode_nth(data, index, "").map(|built| built.chart)
    }

    /// [`Self::decode_variant`] with `title` as the chart's title (a caller gives the file stem: the format has no title).
    ///
    /// # Errors
    /// Returns an error if the file is not an `.ssq` or has no such playable chart.
    pub fn decode_with_title(data: &[u8], index: usize, title: &str) -> RoxResult<RoxChart> {
        decode_nth(data, index, title).map(|built| built.chart)
    }

    /// [`Self::decode_variant`] and what the decoder chose, skipped or repaired.
    ///
    /// # Errors
    /// Returns an error if the file is not an `.ssq` or has no such playable chart.
    pub fn decode_with_report(data: &[u8], index: usize) -> RoxResult<(RoxChart, SsqReport)> {
        decode_nth(data, index, "").map(|built| (built.chart, built.report))
    }

    /// Reads `path` and decodes chart `index` with the file stem as the title.
    ///
    /// # Errors
    /// Returns an error if the file can not be read, is not an `.ssq` or has no such playable chart.
    pub fn decode_file(path: &Path, index: usize) -> RoxResult<RoxChart> {
        Self::decode_with_title(&std::fs::read(path)?, index, &title_of(path))
    }
}

/// The file stem of `path`, the title of its charts.
pub(crate) fn title_of(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn decode_nth(data: &[u8], index: usize, title: &str) -> RoxResult<Built> {
    let mut built = build_all(data, index.saturating_add(1), title)?;
    if index < built.len() {
        let one = built.swap_remove(index);
        one.chart.validate()?;
        Ok(one)
    } else {
        Err(RoxError::InvalidFormat(format!(
            "ssq: {} playable charts, not chart {index}",
            built.len()
        )))
    }
}

/// The file's tempo and its playable step chunks, nothing decoded yet.
fn parse(data: &[u8]) -> RoxResult<Parsed<'_>> {
    if data.len() > MAX_FILE_SIZE {
        return Err(RoxError::InvalidFormat(format!(
            "ssq: file too large: {} bytes (max {MAX_FILE_SIZE})",
            data.len()
        )));
    }
    let walked = walk(data);
    let mut skipped = Vec::new();
    if let Some(reason) = walked.stopped {
        skipped.push(format!("walk stopped: {reason}"));
    }
    if let Some(offset) = walked.sentinel_at {
        skipped.push(format!(
            "walk stopped at offset {offset}: a chunk with param2 0xFFFF ends the lookup, as in the game"
        ));
    }
    let tempo_chunk = walked
        .chunks
        .iter()
        .find(|c| c.kind == kind::TEMPO)
        .ok_or_else(|| RoxError::InvalidFormat("ssq: no tempo chunk (type 1)".to_string()))?;
    let tempo = TempoMap::parse(
        tempo_chunk.param2,
        usize::from(tempo_chunk.param3),
        tempo_chunk.body,
    )?;
    let mut mine_chunks: HashMap<u16, Chunk<'_>> = HashMap::new();
    for chunk in walked.chunks.iter().filter(|c| c.kind == kind::MINES) {
        mine_chunks.entry(chunk.param2).or_insert(*chunk);
    }
    let mut charts: Vec<ChartChunk<'_>> = Vec::new();
    for chunk in &walked.chunks {
        match chunk.kind {
            kind::STEPS => {
                let Some((style, difficulty)) = split_code(chunk.param2) else {
                    skipped.push(format!(
                        "step chunk at offset {}: unsupported difficulty code {:#06x}",
                        chunk.offset, chunk.param2
                    ));
                    continue;
                };
                if charts.iter().any(|c| c.code == chunk.param2) {
                    skipped.push(format!(
                        "step chunk at offset {}: a second chunk for code {:#06x} (the game finds the first)",
                        chunk.offset, chunk.param2
                    ));
                    continue;
                }
                charts.push(ChartChunk {
                    code: chunk.param2,
                    style,
                    difficulty,
                    chunk: *chunk,
                    mines: mine_chunks.get(&chunk.param2).copied(),
                });
            }
            kind::METADATA | kind::LANES => skipped.push(format!(
                "chunk type {} at offset {}: Hudson / metadata chunk, not read",
                chunk.kind, chunk.offset
            )),
            _ => {}
        }
    }
    Ok(Parsed {
        tempo,
        tps: tempo_chunk.param2,
        charts,
        skipped,
    })
}

/// The playable charts in file order, stopping once `want` of them are found.
fn build_all(data: &[u8], want: usize, title: &str) -> RoxResult<Vec<Built>> {
    let parsed = parse(data)?;
    let points = parsed.tempo.bpm_points();
    let mut out: Vec<Built> = Vec::new();
    let mut skipped = parsed.skipped.clone();
    for one in &parsed.charts {
        if out.len() >= want {
            break;
        }
        match build_chart(&parsed, one, &points, title) {
            Ok(built) => out.push(built),
            Err(reason) => skipped.push(format!(
                "{:#06x} ({} {}): {reason}",
                one.code,
                one.style.name(),
                one.difficulty.name()
            )),
        }
    }
    for built in &mut out {
        built.report.skipped.clone_from(&skipped);
    }
    Ok(out)
}

/// One panel's note while the rows are read: where it is in the list and what it is.
#[derive(Debug, Clone, Copy)]
struct Built1 {
    time_us: i64,
    column: u8,
    /// `None`: a tap; `Some(duration)`: a freeze.
    freeze_us: Option<i64>,
    mine: bool,
}

fn build_chart(
    parsed: &Parsed<'_>,
    one: &ChartChunk<'_>,
    points: &[(i64, f32)],
    title: &str,
) -> Result<Built, String> {
    let key_count = one.style.key_count();
    let panel_mask: u8 = if key_count == 8 { 0xFF } else { 0x0F };
    let mut report = SsqReport {
        code: one.code,
        ticks_per_second: parsed.tps,
        instant_tempo_segments: parsed.tempo.instant_segments,
        ..SsqReport::default()
    };
    let mut notes = read_steps(&one.chunk, panel_mask, &parsed.tempo, &mut report)?;
    if let Some(mines) = &one.mines {
        read_mines(mines, panel_mask, &parsed.tempo, &mut notes, &mut report);
    }
    let notes = finish_notes(notes, &mut report);
    if notes.is_empty() {
        return Err("no playable note".to_string());
    }
    let mut rox = RoxChart::new(key_count);
    rox.metadata = Metadata {
        title: title.into(),
        difficulty_name: one.difficulty.name().into(),
        ..Metadata::default()
    };
    let mut holds = 0;
    let mut mines = 0;
    for note in &notes {
        if note.mine {
            mines += 1;
            rox.notes.push(Note::mine(note.time_us, note.column));
        } else if let Some(duration) = note.freeze_us {
            holds += 1;
            rox.notes
                .push(Note::hold(note.time_us, duration, note.column));
        } else {
            rox.notes.push(Note::tap(note.time_us, note.column));
        }
    }
    let first_note = rox.notes.first().map_or(0, |n| n.time_us);
    for (index, &(time, bpm)) in points.iter().enumerate() {
        // The first change sits no later than the first note, as the model requires.
        let time = if index == 0 {
            time.min(first_note)
        } else {
            time
        };
        rox.timing_points.push(TimingPoint::bpm(time, bpm));
    }
    Ok(Built {
        chart: rox,
        report,
        holds,
        mines,
        code: one.code,
        style: one.style,
        difficulty: one.difficulty,
    })
}

/// Whether a time is inside the range the decoder reads.
fn in_range(time_us: i64) -> bool {
    time_us.abs() <= MAX_SECONDS * 1_000_000
}

/// The mask of the panels a shock arrow byte stands for, `None` for a byte that is a normal step.
fn shock_mask(byte: u8) -> Option<u8> {
    matches!(byte, 0x0F | 0xF0 | 0xFF).then_some(byte)
}

/// The notes of a step chunk: rows in file order, shock arrows as mines, freezes resolved from the freeze block.
fn read_steps(
    chunk: &Chunk<'_>,
    panel_mask: u8,
    tempo: &TempoMap,
    report: &mut SsqReport,
) -> Result<Vec<Built1>, String> {
    let body = chunk.body;
    let count = usize::from(chunk.param3);
    let steps_at = count * 4;
    let freeze_at = steps_at + count.next_multiple_of(2);
    if body.len() < freeze_at {
        return Err(format!(
            "the chunk holds {} body bytes, {count} steps need {freeze_at}",
            body.len()
        ));
    }
    let steps = &body[steps_at..steps_at + count];
    #[allow(clippy::naive_bytecount)] // one chunk of at most 65535 bytes
    let freeze_count = steps.iter().filter(|&&byte| byte == 0).count();
    if body.len() < freeze_at + freeze_count * 2 {
        return Err(format!(
            "the chunk holds {} body bytes, {count} steps and {freeze_count} freeze ends need {}",
            body.len(),
            freeze_at + freeze_count * 2
        ));
    }
    let freeze_block = &body[freeze_at..];
    let mut freeze_index = 0;
    let mut notes: Vec<Built1> = Vec::with_capacity(count);
    // Per panel, the latest note on it (index in `notes`), what a freeze end walking backwards finds.
    let mut latest: [Option<usize>; 8] = [None; 8];
    for (i, &byte) in steps.iter().enumerate() {
        let tick = i64::from(i32_at(body, i));
        let freeze = (byte == 0).then(|| {
            let entry = (
                freeze_block[freeze_index * 2],
                freeze_block[freeze_index * 2 + 1],
            );
            freeze_index += 1;
            entry
        });
        if tick < 0 {
            report.negative_ticks += 1;
            continue;
        }
        let time_us = tempo.time_us(tick);
        if !in_range(time_us) {
            report.notes_out_of_range += 1;
            continue;
        }
        if let Some((panels, freeze_kind)) = freeze {
            if freeze_kind != 1 {
                report.ignored_freeze_kinds += 1;
                continue;
            }
            for panel in 0..8u8 {
                if panels & panel_mask & (1 << panel) == 0 {
                    continue;
                }
                let head = latest[usize::from(panel)].map(|index| &mut notes[index]);
                let Some(head) = head.filter(|head| !head.mine) else {
                    report.freeze_ends_without_head += 1;
                    continue;
                };
                let duration = time_us - head.time_us;
                if duration > 0 {
                    head.freeze_us = Some(duration);
                } else {
                    report.empty_freezes_as_taps += 1;
                }
            }
            continue;
        }
        let (bits, mine) = match shock_mask(byte) {
            Some(mask) => {
                report.shock_arrows += 1;
                (mask, true)
            }
            None => (byte, false),
        };
        if bits & !panel_mask != 0 && !mine {
            report.panels_outside_style += 1;
        }
        for panel in 0..8u8 {
            if bits & panel_mask & (1 << panel) == 0 {
                continue;
            }
            latest[usize::from(panel)] = Some(notes.len());
            notes.push(Built1 {
                time_us,
                column: panel,
                freeze_us: None,
                mine,
            });
        }
    }
    Ok(notes)
}

/// The mine chunk (type 20) of a chart: `{i32 tick, u8 panels, u8 flags, u16 reserved}` entries.
fn read_mines(
    chunk: &Chunk<'_>,
    panel_mask: u8,
    tempo: &TempoMap,
    notes: &mut Vec<Built1>,
    report: &mut SsqReport,
) {
    let body = chunk.body;
    // The entry count is the header's; a body too short for it holds fewer entries.
    let count = usize::from(chunk.param3).min(body.len() / 8);
    for i in 0..count {
        let at = i * 8;
        let tick = i64::from(i32_at(body, i * 2));
        let (panels, flags) = (body[at + 4], body[at + 5]);
        let reserved = u16::from_le_bytes([body[at + 6], body[at + 7]]);
        let valid = flags == 0
            && reserved == 0
            && tick >= 0
            && panels != 0
            && shock_mask(panels).is_none()
            && panels & !panel_mask == 0;
        if !valid {
            report.invalid_mine_entries += 1;
            continue;
        }
        let time_us = tempo.time_us(tick);
        if !in_range(time_us) {
            report.notes_out_of_range += 1;
            continue;
        }
        for panel in 0..8u8 {
            if panels & (1 << panel) != 0 {
                notes.push(Built1 {
                    time_us,
                    column: panel,
                    freeze_us: None,
                    mine: true,
                });
            }
        }
        report.mines_from_chunk += 1;
    }
}

/// Sorts the notes by time and column and drops the ones inside a freeze of their column (the model forbids them).
fn finish_notes(mut notes: Vec<Built1>, report: &mut SsqReport) -> Vec<Built1> {
    notes.sort_by(|a, b| {
        a.time_us
            .cmp(&b.time_us)
            .then(a.column.cmp(&b.column))
            .then(b.freeze_us.is_some().cmp(&a.freeze_us.is_some()))
    });
    let mut busy_until = [i64::MIN; 8];
    let mut kept = Vec::with_capacity(notes.len());
    for note in notes {
        let column = usize::from(note.column);
        if note.time_us < busy_until[column] {
            report.overlapping_notes_dropped += 1;
            continue;
        }
        busy_until[column] = note.time_us + note.freeze_us.unwrap_or(0);
        kept.push(note);
    }
    kept
}
