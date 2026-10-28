#![warn(clippy::pedantic)]
//! `.ksf` parser: a Kick It Up / Direct Move step file (one chart) into the types of the `.sm`
//! reader, so that the timing (BPM changes, delays, the start time) and the columns go through
//! the one pipeline of [`crate::formats::sm`].
//!
//! Ported from StepMania's `NotesLoaderKSF.cpp` (file-name classification, the `#BUNKI` BPM
//! changes, the step rows, the hold rule, the Direct Move `|` rows); see `mod.rs` for the
//! decisions.
// Adapted from StepMania (MIT), src/NotesLoaderKSF.cpp, (c) 2001-2006 Chris Danford, Glenn Maynard,
// Jason Felds; their permission notice is reproduced in VENDORED.md.
use rox::error::{RoxError, RoxResult};

use crate::formats::sm::legacy::{
    beat_to_row, decode_text, hhmmss_to_seconds, row_to_beat, string_to_float, string_to_int,
};
use crate::formats::sm::msd::{Tag, tokenize_raw};
use crate::formats::sm::ssc_parser::{SongTiming, bpm_points};
use crate::formats::sm::timing_data::TimingData;
use crate::formats::sm::types::{SmChart, SmFile, SmMetadata, SmNote, SmNoteType};

const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// The end-of-chart row.
const END_ROW: &str = "2222222222222";

/// Where a `.ksf` comes from: a KSF has no tag for its step type and difficulty, they are in the
/// FILE NAME, and the artist/title fall back to the folder name.
#[derive(Debug, Clone, Copy, Default)]
pub struct KsfContext<'a> {
    /// The name of the file, with or without extension and folder (`Crazy_1.ksf`, `Double.ksf`).
    /// Empty when unknown: the chart is then a `pump-single` at difficulty Hard (10), unless
    /// `#PLAYER` says `double`.
    pub file_name: &'a str,
    /// The name of the song folder (`Title`, `Artist - Title` or `Artist - Title - Difficulty`),
    /// used when `#TITLE` gives no title or no artist.
    pub folder_name: Option<&'a str>,
}

/// A parsed file and what the parser repaired or ignored while reading it.
#[derive(Debug, Clone, Default)]
pub struct Parsed {
    /// The chart when it can be played (`file.skipped` says why otherwise).
    pub file: SmFile,
    /// One line per thing that was dropped or repaired.
    pub warnings: Vec<String>,
    /// Whether the file counts as "KIU compliant" (header BPM changes, no Direct Move rows).
    pub kiu_compliant: bool,
}

/// Parses a `.ksf` file.
///
/// A chart that can not be played (`pump-double` and `pump-couple` have 10 columns, rows too
/// short, a tick count of 0 or less, timing that can not be honoured) is left out and described
/// in [`SmFile::skipped`].
///
/// # Errors
/// Returns an error if the data exceeds 100 MB.
pub fn parse_detailed(data: &[u8], context: &KsfContext<'_>) -> RoxResult<Parsed> {
    if data.len() > MAX_FILE_SIZE {
        return Err(RoxError::InvalidFormat(format!(
            "File too large: {} bytes (max 100MB)",
            data.len()
        )));
    }
    let text = decode_text(data, true);
    let tags = tokenize_raw(&text);
    let rows = step_rows(&tags);
    let kiu_compliant = is_kiu_compliant(&tags, &rows);
    let mut parsed = Parsed {
        kiu_compliant,
        ..Parsed::default()
    };
    match read_chart(&tags, &rows, kiu_compliant, context, &mut parsed.warnings) {
        Ok((chart, offset_us)) => {
            parsed.file = SmFile {
                metadata: metadata_of(&tags, context),
                offset_us,
                bpms: chart.bpms.clone().unwrap_or_default(),
                charts: vec![chart],
                skipped: Vec::new(),
            };
        }
        Err(reason) => {
            parsed.file.metadata = metadata_of(&tags, context);
            parsed.file.skipped.push(reason);
        }
    }
    Ok(parsed)
}

/// Parses a `.ksf` file into the `.sm` types (see [`parse_detailed`]).
///
/// # Errors
/// Returns an error if the data exceeds 100 MB.
pub fn parse(data: &[u8], context: &KsfContext<'_>) -> RoxResult<SmFile> {
    parse_detailed(data, context).map(|parsed| parsed.file)
}

/// The rows of the last `#STEP` tag, trimmed, without empty lines.
fn step_rows(tags: &[Tag]) -> Vec<String> {
    tags.iter()
        .rev()
        .find(|tag| tag.name == "STEP")
        .map(|tag| {
            tag.text()
                .lines()
                .map(str::trim)
                .filter(|row| !row.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The loader's `LoadGlobalData` test: a header BPM change (`#BPM2`, `#BPM3`, `#BUNKI`,
/// `#BUNKI2`, `#STARTTIME2`) makes a file KIU compliant; so does a chart that reaches its end row
/// with no Direct Move (`|`) row before.
fn is_kiu_compliant(tags: &[Tag], rows: &[String]) -> bool {
    if tags.iter().any(|tag| {
        matches!(
            tag.name.as_str(),
            "BPM2" | "BPM3" | "BUNKI" | "BUNKI2" | "STARTTIME2"
        )
    }) {
        return true;
    }
    let mut direct_move = false;
    for row in rows {
        if row == END_ROW {
            return !direct_move;
        }
        if row.starts_with('|') {
            direct_move = true;
        }
    }
    false
}

/// `LoadTags`: `Title`, `Artist - Title` or `Artist - Title - Difficulty` into (title, artist);
/// anything else gives none.
fn title_and_artist(text: &str) -> (String, String) {
    let mut bits: Vec<&str> = text.split(" - ").collect();
    if bits.len() == 3
        && ["double", "easy", "normal", "hard", "crazy", "nightmare"]
            .iter()
            .any(|name| bits[2].eq_ignore_ascii_case(name))
    {
        bits.truncate(2);
    }
    match bits.as_slice() {
        [title] => ((*title).to_string(), String::new()),
        [artist, title] => ((*title).to_string(), (*artist).to_string()),
        _ => (String::new(), String::new()),
    }
}

fn last<'a>(tags: &'a [Tag], name: &str) -> Option<&'a Tag> {
    tags.iter().rev().find(|tag| tag.name == name)
}

fn metadata_of(tags: &[Tag], context: &KsfContext<'_>) -> SmMetadata {
    let mut metadata = SmMetadata::default();
    let (mut title, mut artist) = last(tags, "TITLE")
        .map(|tag| title_and_artist(&tag.text()))
        .unwrap_or_default();
    if let Some(folder) = context.folder_name {
        let (folder_title, folder_artist) = title_and_artist(folder);
        if title.is_empty() {
            title = folder_title;
        }
        if artist.is_empty() {
            artist = folder_artist;
        }
    }
    metadata.title = title;
    metadata.artist = artist;
    metadata.music = last(tags, "SONGFILE").map(Tag::text).unwrap_or_default();
    metadata.background = last(tags, "TITLEFILE").map(Tag::text).unwrap_or_default();
    metadata.banner = last(tags, "DISCFILE").map(Tag::text).unwrap_or_default();
    metadata.sample_start = tags
        .iter()
        .rev()
        .find(|tag| tag.name == "MUSICINTRO" || tag.name == "INTRO")
        .map_or(0.0, |tag| f64::from(hhmmss_to_seconds(&tag.text())));
    // "Intro length in piu mixes is generally 7 seconds" (the loader's constant).
    metadata.sample_length = 7.0;
    metadata
}

/// The step type and difficulty the loader gives a chart from its file name (lower-cased, no
/// extension).
struct Classified {
    difficulty: &'static str,
    default_meter: u32,
    stepstype: &'static str,
}

fn classify(name: &str, player_is_double: bool) -> Classified {
    let has = |parts: &[&str]| parts.iter().any(|part| name.contains(part));
    let (difficulty, default_meter) = if has(&["another"]) {
        ("Edit", 25)
    } else if has(&["wild", "wd", "crazy+", "cz+", "hardcore"]) {
        ("Challenge", 20)
    } else if has(&["crazy", "cz", "nightmare", "nm", "crazydouble"]) {
        ("Hard", 14)
    } else if has(&["hard", "hd", "freestyle", "fs", "double"]) {
        ("Medium", 8)
    } else if has(&["easy", "ez", "normal"]) {
        ("Easy", 4)
    } else if has(&["beginner", "practice", "pr"]) {
        ("Beginner", 4)
    } else {
        ("Hard", 10)
    };
    // `halfdouble` is checked before `double`.
    let stepstype = if has(&["halfdouble", "half-double", "h_double", "hdb"]) {
        "pump-halfdouble"
    } else if player_is_double || has(&["double", "nightmare", "freestyle", "db", "nm", "fs"]) {
        "pump-double"
    } else if has(&["_1"]) {
        "pump-single"
    } else if has(&["_2"]) {
        "pump-couple"
    } else {
        "pump-single"
    };
    Classified {
        difficulty,
        default_meter,
        stepstype,
    }
}

/// The name of a file without folder and extension, lower-cased (`splitpath` + `MakeLower`).
fn base_name(file_name: &str) -> String {
    let name = file_name.rsplit(['/', '\\']).next().unwrap_or("");
    let name = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    name.to_lowercase()
}

/// BPM changes of a chart by row; a later value on the same row replaces the earlier one.
#[derive(Default)]
struct Segments {
    bpms: Vec<(i64, f32)>,
    delays: Vec<(i64, f32)>,
}

impl Segments {
    fn set_bpm(&mut self, row: i64, bpm: f32) {
        match self.bpms.iter_mut().find(|(at, _)| *at == row) {
            Some(entry) => entry.1 = bpm,
            None => self.bpms.push((row, bpm)),
        }
    }

    fn bpm_at(&self, row: i64) -> f32 {
        self.bpms
            .iter()
            .filter(|(at, _)| *at <= row)
            .max_by_key(|(at, _)| *at)
            .map_or(0.0, |&(_, bpm)| bpm)
    }

    fn delay_at(&self, row: i64) -> f32 {
        self.delays
            .iter()
            .find(|(at, _)| *at == row)
            .map_or(0.0, |&(_, delay)| delay)
    }

    fn set_delay(&mut self, row: i64, delay: f32) {
        match self.delays.iter_mut().find(|(at, _)| *at == row) {
            Some(entry) => entry.1 = delay,
            None => self.delays.push((row, delay)),
        }
    }

    /// `HandleBunki`: the BPM `current` takes over at `position` seconds of the audio (`gap` =
    /// -start time), computed at the tempo `early` the way the loader does (a quirk: it assumes
    /// `early` ran from beat 0).
    fn add_bunki(&mut self, early: f32, current: f32, gap: f32, position: f32) {
        let beat = (position + gap) * (early / 60.0);
        self.set_bpm(beat_to_row(beat), current);
    }
}

/// What the Direct Move rows of a chart asked for and the loader does not use.
#[derive(Default)]
struct Ignored {
    invalid_cells: usize,
    direct_move_rows: usize,
    open_holds: usize,
}

#[allow(clippy::too_many_lines)] // one pass over the tags, then one over the rows, as the loader
fn read_chart(
    tags: &[Tag],
    rows: &[String],
    mut kiu_compliant: bool,
    context: &KsfContext<'_>,
    warnings: &mut Vec<String>,
) -> Result<(SmChart, i64), String> {
    let (mut gap1, mut gap2) = (0.0_f32, 0.0_f32);
    let (mut bpm1, mut bpm2, mut bpm3) = (-1.0_f32, -1.0_f32, -1.0_f32);
    let (mut pos2, mut pos3) = (-1.0_f32, -1.0_f32);
    let mut start_time_cs = 0.0_f32;
    let mut tick_count: Option<i32> = None;
    let mut meter: Option<u32> = None;
    let mut player_is_double = false;
    let mut segments = Segments::default();
    for tag in tags {
        let name = tag.name.as_str();
        if name == "TITLE" || name.ends_with("INTRO") || name.ends_with("FILE") {
            continue;
        }
        let value = tag.first();
        match name {
            "BPM" => {
                bpm1 = string_to_float(value);
                segments.set_bpm(0, bpm1);
            }
            "BPM2" if kiu_compliant => bpm2 = string_to_float(value),
            "BPM3" if kiu_compliant => bpm3 = string_to_float(value),
            "BUNKI" if kiu_compliant => pos2 = string_to_float(value) / 100.0,
            "BUNKI2" if kiu_compliant => pos3 = string_to_float(value) / 100.0,
            "STARTTIME" => {
                start_time_cs = string_to_float(value);
                gap1 = -start_time_cs / 100.0;
            }
            "STARTTIME2" if kiu_compliant => gap2 = -string_to_float(value) / 100.0,
            "STARTTIME3" => kiu_compliant = true,
            "TICKCOUNT" => {
                let ticks = string_to_int(value);
                if ticks <= 0 {
                    return Err(format!("invalid tick count {ticks}"));
                }
                tick_count = Some(ticks);
            }
            "DIFFICULTY" => meter = Some(u32::try_from(string_to_int(value).max(1)).unwrap_or(1)),
            "PLAYER" => player_is_double |= value.to_lowercase().contains("double"),
            _ => {}
        }
    }
    let mut ticks = tick_count.unwrap_or_else(|| {
        warnings.push("no #TICKCOUNT: 4 assumed".to_string());
        4
    });
    if kiu_compliant {
        if bpm2 > 0.0 && pos2 > 0.0 {
            segments.add_bunki(bpm1, bpm2, gap1, pos2);
        }
        if bpm3 > 0.0 && pos3 > 0.0 {
            segments.add_bunki(bpm2, bpm3, gap2, pos3);
        }
    }

    let name = base_name(context.file_name);
    let class = classify(&name, player_is_double);
    let columns: usize = match class.stepstype {
        "pump-single" => 5,
        "pump-halfdouble" => 6,
        other => {
            return Err(format!(
                "unsupported step type `{other}` (10 columns; ROX plays 4 to 7)"
            ));
        }
    };
    // `halfdouble` is the middle six columns of the ten.
    let skip = if class.stepstype == "pump-halfdouble" {
        2
    } else {
        0
    };

    let mut notes: Vec<(i64, usize, SmNoteType)> = Vec::new();
    let mut ignored = Ignored::default();
    let mut hold_start: Vec<Option<i64>> = vec![None; columns];
    let (mut current, mut previous) = (0.0_f64, 0.0_f64);
    let row_of = |beat: f64| {
        #[allow(clippy::cast_possible_truncation)] // a chart is far shorter than f32's range
        beat_to_row(beat as f32)
    };
    for row in rows {
        if row == END_ROW {
            for (column, start) in hold_start.iter_mut().enumerate() {
                if let Some(start) = start.take() {
                    close_hold(&mut notes, column, start, row_of(previous));
                }
            }
            break;
        }
        if row.starts_with('|') {
            direct_move_row(
                row,
                &mut ticks,
                row_of(current),
                &mut segments,
                &mut ignored,
            )?;
            continue;
        }
        let cells = row.as_bytes().get(skip..).unwrap_or_default();
        if cells.len() < columns {
            return Err(format!(
                "row `{row}` has {} columns, expected {}",
                cells.len() + skip,
                columns + skip
            ));
        }
        let row_now = row_of(current);
        for (column, &cell) in cells[..columns].iter().enumerate() {
            if cell == b'4' {
                // The head is the first `4`; the middle is ignored.
                hold_start[column].get_or_insert(row_now);
                continue;
            }
            if let Some(start) = hold_start[column].take() {
                close_hold(&mut notes, column, start, row_of(previous));
            }
            match cell {
                // A fake is never judged.
                b'0' | b'F' | b'f' => {}
                b'1' => notes.push((row_now, column, SmNoteType::Tap)),
                b'M' | b'm' => notes.push((row_now, column, SmNoteType::Mine)),
                b'L' | b'l' => notes.push((row_now, column, SmNoteType::Lift)),
                _ => ignored.invalid_cells += 1,
            }
        }
        previous = current;
        current = previous + 1.0 / f64::from(ticks);
    }
    ignored.open_holds = hold_start.iter().flatten().count();
    for (count, what) in [
        (ignored.invalid_cells, "invalid step characters: ignored"),
        (
            ignored.open_holds,
            "holds never closed by the end row: removed",
        ),
        (
            ignored.direct_move_rows,
            "Direct Move rows (|M |C |S |F |X) with no effect on time: ignored",
        ),
    ] {
        if count > 0 {
            warnings.push(format!("{count} {what}"));
        }
    }

    let bpm_entries: Vec<(f64, f64)> = segments
        .bpms
        .iter()
        .map(|&(row, bpm)| (row_to_beat(row), f64::from(bpm)))
        .collect();
    let delay_entries: Vec<(f64, f64)> = segments
        .delays
        .iter()
        .map(|&(row, delay)| (row_to_beat(row), f64::from(delay)))
        .collect();
    let (bpm_list, timing) = SongTiming::new(bpm_entries, Vec::new(), delay_entries)
        .timing_for(&[])
        .map_err(|reason| format!("{}: {reason}", class.stepstype))?;
    let chart = SmChart {
        stepstype: class.stepstype.to_string(),
        difficulty: class.difficulty.to_string(),
        meter: meter.unwrap_or(class.default_meter),
        #[allow(clippy::cast_possible_truncation)] // 5 or 6
        column_count: columns as u8,
        notes: place(&notes, &timing),
        description: name,
        credit: None,
        bpms: Some(bpm_points(&bpm_list, &timing)),
        offset_us: None,
    };
    #[allow(clippy::cast_possible_truncation)] // centiseconds to µs of a sane start time
    let offset_us = (-f64::from(start_time_cs) * 10_000.0).round() as i64;
    Ok((chart, offset_us))
}

/// A hold from `start` to `end`; a hold of a single row is a tap.
fn close_hold(notes: &mut Vec<(i64, usize, SmNoteType)>, column: usize, start: i64, end: i64) {
    if start == end {
        notes.push((start, column, SmNoteType::Tap));
    } else {
        notes.push((start, column, SmNoteType::HoldHead));
        notes.push((end, column, SmNoteType::Tail));
    }
}

/// A Direct Move row: `|T<n>|` tick count, `|B<n>|` BPM at the current beat, `|E<n>|` a delay of
/// `n` ticks at the current BPM (added to the delay already there), `|D<ms>|` a delay of `ms`
/// milliseconds. `|M`/`|C` (combo), `|S` (speed), `|F`, `|X` (scroll) do not move time: counted.
fn direct_move_row(
    row: &str,
    ticks: &mut i32,
    row_now: i64,
    segments: &mut Segments,
    ignored: &mut Ignored,
) -> Result<(), String> {
    let number = row
        .get(2..row.len().saturating_sub(1))
        .map_or(0.0, string_to_float);
    match row.as_bytes().get(1) {
        Some(b'T') => {
            #[allow(clippy::cast_possible_truncation)] // the loader truncates
            let value = number as i32;
            if value <= 0 {
                return Err(format!("invalid Direct Move tick count in `{row}`"));
            }
            *ticks = value;
        }
        Some(b'B') => segments.set_bpm(row_now, number),
        Some(b'E') => {
            #[allow(clippy::cast_precision_loss)] // tick counts are small
            let ticks = *ticks as f32;
            let delay =
                60.0 / segments.bpm_at(row_now) * number / ticks + segments.delay_at(row_now);
            segments.set_delay(row_now, delay);
        }
        // The loader adds the *stop* at the row to the milliseconds, and a KSF has no stops.
        Some(b'D') => segments.set_delay(row_now, number / 1000.0),
        _ => ignored.direct_move_rows += 1,
    }
    Ok(())
}

fn place(notes: &[(i64, usize, SmNoteType)], timing: &TimingData) -> Vec<SmNote> {
    notes
        .iter()
        .map(|&(row, column, note_type)| SmNote {
            time_us: timing.note_us(row_to_beat(row)),
            #[allow(clippy::cast_possible_truncation)] // 5 or 6 columns
            column: column as u8,
            note_type,
        })
        .collect()
}
