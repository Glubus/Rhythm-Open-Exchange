#![warn(clippy::pedantic)]
//! `<song>-chart.json` + `<song>-metadata.json` to a four-column ROX chart, plus the audio plan. See `mod.rs` for the decisions.

use std::collections::BTreeMap;

use rox::audio_plan::AudioPlan;
use rox::error::RoxResult;
use rox::model::{Metadata, Note, RoxChart};

use super::audio::{self, Stems};
use super::data::{Loaded, bad, load, ms_to_us};

/// Columns of a V-Slice chart: the player's four arrows.
const KEYS: u8 = 4;

/// The difficulty ids of the base game, in the game's order (`Constants.DEFAULT_DIFFICULTY_LIST_FULL`). A chart lists them first,
/// in this order; every other id follows, sorted by name.
pub const DIFFICULTY_ORDER: [&str; 5] = ["easy", "normal", "hard", "erect", "nightmare"];

/// Decoder of V-Slice charts: the chart file and its metadata file, given together as bytes.
pub struct VSliceDecoder;

/// A playable difficulty of one chart file. A difficulty with no player note is not a variant, so `index` counts the
/// playable ones from 0 in the order of [`DIFFICULTY_ORDER`] then by name.
#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    /// Number to give to [`VSliceDecoder::decode_variant`]: `0..variants.len()`.
    pub index: usize,
    /// The difficulty id, the key of the `notes` map (`hard`, `erect`...).
    pub difficulty: String,
    /// The id in Title Case (`Hard`), also the decoded chart's `difficulty_name`.
    pub name: String,
    /// The author's Freeplay rating (`playData.ratings`), when written.
    pub rating: Option<f64>,
    /// Notes of the decoded chart (a hold is one).
    pub notes: usize,
    pub holds: usize,
    pub key_count: u8,
    pub duration_us: i64,
}

/// What the decoder dropped, ignored or repaired for one difficulty. Nothing here changes the chart; it explains it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VSliceReport {
    /// The difficulty id.
    pub difficulty: String,
    /// `scrollSpeed` of the difficulty (else of `default`): a constant display speed, NOT put in the chart (no SV point).
    pub scroll_speed: Option<f64>,
    /// Notes with `d` 4 and above (the opponent and further strumlines): not in the chart.
    pub other_strumline_notes: usize,
    /// Notes with a negative `d`: skipped.
    pub invalid_data_notes: usize,
    /// Player notes with a time or a length that is out of range: skipped.
    pub notes_out_of_range: usize,
    /// Player notes before time 0: skipped.
    pub notes_before_zero: usize,
    /// Player notes with a kind (`k`): decoded as normal notes. Name -> count.
    pub note_kinds: BTreeMap<String, usize>,
    /// Holds shorter than a microsecond: taps.
    pub empty_holds_as_taps: usize,
    /// Notes that start inside a hold or at the same time on the same column as another note: dropped.
    pub overlapping_notes_dropped: usize,
    /// Song events of the chart (`events`): not representable, ignored.
    pub events_ignored: usize,
    /// `timeChanges` given out of order: sorted.
    pub unsorted_time_changes: bool,
    /// `timeChanges` before time 0: moved to 0.
    pub negative_time_changes_clamped: usize,
    /// `timeChanges` at the same time as a later one: dropped.
    pub same_time_changes_replaced: usize,
    /// `timeChanges` that change neither the BPM nor the numerator: dropped.
    pub repeated_time_changes: usize,
    /// `timeChanges` with a denominator other than 4: ROX keeps only the numerator.
    pub non_quarter_denominators: usize,
    /// File or character names refused when the audio plan was built (a separator, `..`, absolute...).
    pub unsafe_names: usize,
}

impl VSliceReport {
    /// Player notes decoded as normal notes although they carry a kind.
    #[must_use]
    pub fn note_kinds_ignored(&self) -> usize {
        self.note_kinds.values().sum()
    }
}

/// A difficulty, converted.
pub(super) struct Built {
    pub chart: RoxChart,
    pub report: VSliceReport,
    pub holds: usize,
}

impl VSliceDecoder {
    /// The playable difficulties of the chart file, in the stable order of [`DIFFICULTY_ORDER`] then by name.
    ///
    /// # Errors
    /// Returns an error if a file is not JSON, has an unsupported version, a `timeFormat` other than `ms`, or no valid tempo map.
    pub fn variants(chart: &[u8], metadata: &[u8]) -> RoxResult<Vec<Variant>> {
        let loaded = load(chart, metadata)?;
        variants_of(&loaded)
    }

    /// Decodes playable difficulty `index` (the numbering of [`Self::variants`]).
    ///
    /// # Errors
    /// Returns an error if the files do not decode or there is no such playable difficulty.
    pub fn decode_variant(chart: &[u8], metadata: &[u8], index: usize) -> RoxResult<RoxChart> {
        Self::decode_with_report(chart, metadata, index).map(|(chart, _)| chart)
    }

    /// [`Self::decode_variant`] and what the decoder dropped, ignored or repaired.
    ///
    /// # Errors
    /// Returns an error if the files do not decode or there is no such playable difficulty.
    pub fn decode_with_report(
        chart: &[u8],
        metadata: &[u8],
        index: usize,
    ) -> RoxResult<(RoxChart, VSliceReport)> {
        let loaded = load(chart, metadata)?;
        let built = build_nth(&loaded, index)?;
        Ok((built.chart, built.report))
    }

    /// [`Self::decode_with_report`] plus the [`AudioPlan`]: the instrumental and the vocal stems as whole-file events, named
    /// as the game names them (`Inst.ogg` or `Inst-<id>.ogg`, `Voices-<character>[-<variation>].ogg`), shifted by the
    /// metadata `offsets`. `variation` is the variation of the files given (`"default"` or `""` for the default one).
    /// The files are not visible here, so the vocals are the characters' own files, all `optional`; use
    /// [`Self::decode_with_audio_plan_in`] when the folder can be asked which files exist.
    ///
    /// # Errors
    /// Returns an error if the files do not decode, there is no such playable difficulty, or an offset is out of range.
    pub fn decode_with_audio_plan(
        chart: &[u8],
        metadata: &[u8],
        index: usize,
        variation: &str,
    ) -> RoxResult<(RoxChart, VSliceReport, AudioPlan)> {
        plan_nth(chart, metadata, index, variation, None)
    }

    /// [`Self::decode_with_audio_plan`] resolving the vocals as the game does: `exists(name)` says whether a stem file
    /// (`Voices-bf.ogg`, a bare file name in the song's folder) is there; the lists `playerVocals` / `opponentVocals` are used
    /// when all their files exist, else the character id is shortened one `-` part at a time (`bf-car` -> `bf`), and the
    /// plain `Voices[-<variation>].ogg` is the last resort when nothing was found.
    ///
    /// # Errors
    /// As [`Self::decode_with_audio_plan`].
    pub fn decode_with_audio_plan_in(
        chart: &[u8],
        metadata: &[u8],
        index: usize,
        variation: &str,
        exists: &dyn Fn(&str) -> bool,
    ) -> RoxResult<(RoxChart, VSliceReport, AudioPlan)> {
        let lookup = |name: &str| exists(name).then_some(0);
        plan_nth(
            chart,
            metadata,
            index,
            variation,
            Some(&Stems {
                lookup: &lookup,
                package: None,
            }),
        )
    }
}

pub(super) fn plan_nth(
    chart: &[u8],
    metadata: &[u8],
    index: usize,
    variation: &str,
    stems: Option<&Stems<'_>>,
) -> RoxResult<(RoxChart, VSliceReport, AudioPlan)> {
    let loaded = load(chart, metadata)?;
    let mut built = build_nth(&loaded, index)?;
    let plan = audio::build_plan(
        &loaded.meta,
        variation,
        stems,
        &mut built.report.unsafe_names,
    )?;
    Ok((built.chart, built.report, plan))
}

fn variants_of(loaded: &Loaded) -> RoxResult<Vec<Variant>> {
    let mut out = Vec::new();
    for (index, id) in playable_difficulties(loaded).into_iter().enumerate() {
        let built = build(loaded, &id)?;
        out.push(Variant {
            index,
            name: built.chart.metadata.difficulty_name.to_string(),
            rating: rating(loaded, &id),
            notes: built.chart.notes.len(),
            holds: built.holds,
            key_count: KEYS,
            duration_us: built.chart.duration_us(),
            difficulty: id,
        });
    }
    Ok(out)
}

pub(super) fn build_nth(loaded: &Loaded, index: usize) -> RoxResult<Built> {
    let ids = playable_difficulties(loaded);
    let Some(id) = ids.get(index) else {
        return Err(bad(format!(
            "{} playable difficulties, not difficulty {index}",
            ids.len()
        )));
    };
    let built = build(loaded, id)?;
    built.chart.validate()?;
    Ok(built)
}

/// The ids of the difficulties that have a player note, in the documented order.
fn playable_difficulties(loaded: &Loaded) -> Vec<String> {
    let mut ids: Vec<&String> = loaded
        .chart
        .notes
        .iter()
        .filter(|(_, notes)| notes.iter().any(|n| (0..i64::from(KEYS)).contains(&n.d)))
        .map(|(id, _)| id)
        .collect();
    let rank = |id: &str| {
        DIFFICULTY_ORDER
            .iter()
            .position(|known| *known == id)
            .unwrap_or(DIFFICULTY_ORDER.len())
    };
    ids.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.cmp(b)));
    ids.into_iter().cloned().collect()
}

fn rating(loaded: &Loaded, id: &str) -> Option<f64> {
    loaded
        .meta
        .play_data
        .as_ref()?
        .ratings
        .as_ref()?
        .get(id)
        .copied()
}

/// `hard` -> `Hard`, `my-diff` -> `My Diff`: each word (split on `-`, `_`, space) gets a capital first letter.
fn title_case(id: &str) -> String {
    id.split(['-', '_', ' '])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn build(loaded: &Loaded, id: &str) -> RoxResult<Built> {
    let source = loaded
        .chart
        .notes
        .get(id)
        .ok_or_else(|| bad(format!("no difficulty {id:?}")))?;
    let mut report = VSliceReport {
        difficulty: id.to_string(),
        scroll_speed: loaded
            .chart
            .scroll_speed
            .get(id)
            .or_else(|| loaded.chart.scroll_speed.get("default"))
            .copied(),
        events_ignored: loaded.chart.events.len(),
        unsorted_time_changes: loaded.tempo.unsorted_time_changes,
        negative_time_changes_clamped: loaded.tempo.negative_time_changes_clamped,
        same_time_changes_replaced: loaded.tempo.same_time_changes_replaced,
        repeated_time_changes: loaded.tempo.repeated_time_changes,
        non_quarter_denominators: loaded.tempo.non_quarter_denominators,
        ..VSliceReport::default()
    };

    let mut notes: Vec<Note> = Vec::with_capacity(source.len());
    for raw in source {
        let Ok(column) = u8::try_from(raw.d) else {
            report.invalid_data_notes += usize::from(raw.d < 0);
            report.other_strumline_notes += usize::from(raw.d >= 0);
            continue;
        };
        if column >= KEYS {
            report.other_strumline_notes += 1;
            continue;
        }
        let (Some(time_us), Some(length_us)) = (ms_to_us(raw.t), ms_to_us(raw.l.max(0.0))) else {
            report.notes_out_of_range += 1;
            continue;
        };
        if time_us < 0 {
            report.notes_before_zero += 1;
            continue;
        }
        if let Some(kind) = raw.k.as_deref().filter(|k| !k.is_empty()) {
            *report.note_kinds.entry(kind.to_string()).or_default() += 1;
        }
        if raw.l > 0.0 && length_us == 0 {
            report.empty_holds_as_taps += 1;
        }
        notes.push(if length_us > 0 {
            Note::hold(time_us, length_us, column)
        } else {
            Note::tap(time_us, column)
        });
    }
    notes.sort_by_key(|note| (note.time_us, u8::from(!note.is_hold()), note.column));
    let mut last_end = [i64::MIN; KEYS as usize];
    let mut last_start = [i64::MIN; KEYS as usize];
    let mut kept = Vec::with_capacity(notes.len());
    let mut holds = 0usize;
    for note in notes {
        let column = usize::from(note.column);
        if note.time_us < last_end[column] || note.time_us == last_start[column] {
            report.overlapping_notes_dropped += 1;
            continue;
        }
        last_end[column] = note.end_time_us();
        last_start[column] = note.time_us;
        holds += usize::from(note.is_hold());
        kept.push(note);
    }

    let mut chart = RoxChart::new(KEYS);
    chart.metadata = metadata(loaded, id);
    chart.timing_points.clone_from(&loaded.timing);
    chart.notes = kept;
    Ok(Built {
        chart,
        report,
        holds,
    })
}

fn metadata(loaded: &Loaded, id: &str) -> Metadata {
    let meta = &loaded.meta;
    let play = meta.play_data.as_ref();
    let preview_start = play.and_then(|p| p.preview_start).and_then(ms_to_us);
    let preview_end = play.and_then(|p| p.preview_end).and_then(ms_to_us);
    let defaults = Metadata::default();
    #[allow(clippy::cast_possible_truncation)] // a rating is a small integer
    let difficulty_value = rating(loaded, id).map(|r| r as f32);
    Metadata {
        title: meta.song_name.as_deref().unwrap_or("Unknown").into(),
        artist: meta.artist.as_deref().unwrap_or("Unknown").into(),
        creator: meta.charter.as_deref().unwrap_or("").into(),
        difficulty_name: title_case(id).into(),
        difficulty_value,
        // The audio is several stems: the audio plan names them, the chart has no audio file.
        audio_file: "".into(),
        preview_time_us: preview_start.map_or(defaults.preview_time_us, |t| t.max(0)),
        preview_duration_us: match (preview_start, preview_end) {
            (Some(start), Some(end)) if end > start => end - start,
            _ => defaults.preview_duration_us,
        },
        source: Some("Friday Night Funkin'".into()),
        tags: vec!["fnf".into()],
        ..defaults
    }
}
