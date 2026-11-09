#![warn(clippy::pedantic)]
//! The audio plan: instrumental and vocal stems, named and shifted as the game does (`Song.hx`, `Paths.hx`, `PlayState.hx`).

use rox::audio_plan::{AudioPlan, SoundEvent};
use rox::error::RoxResult;

use super::data::{MetadataFile, Offsets, bad, ms_to_us};
use super::files::suffix;
use crate::formats::bms::bmson::safe_name;

/// Where the stems are: what to ask about a file, and whether the events point inside a package.
pub(super) struct Stems<'a> {
    /// `Some(index)` when the stem file exists; the index is its entry number in a package (0 for loose files).
    pub lookup: &'a dyn Fn(&str) -> Option<u32>,
    /// The package file name: events are then `SoundEvent::in_container` events on it.
    pub package: Option<&'a str>,
}

/// One path component of a file name: no separator, no NUL, not empty.
fn component(id: &str) -> bool {
    !id.is_empty() && !id.contains(['/', '\\', '\0'])
}

fn voice_name(character: &str, suffix: &str) -> String {
    format!("Voices-{character}{suffix}.ogg")
}

/// `bf-car` gives `bf`; `bf` gives the empty string.
fn shorten(id: &str) -> &str {
    id.rsplit_once('-').map_or("", |(head, _)| head)
}

/// The vocal files of one side. `list` is `playerVocals` / `opponentVocals`. Without `stems` the listed ids (or the
/// character) are named as they are; with `stems` the game's search runs (`buildPlayerVoiceList`): the list when every file
/// exists, else the character id shortened until a file exists, first with the variation suffix then without it (the game
/// then keeps shortening WITH the suffix, a quirk reproduced here).
fn voice_files(
    list: Option<&[String]>,
    character: &str,
    suffix: &str,
    stems: Option<&Stems<'_>>,
    refused: &mut usize,
) -> Vec<String> {
    let mut usable = |id: &str| {
        let ok = component(id);
        *refused += usize::from(!ok);
        ok
    };
    let Some(stems) = stems else {
        return match list {
            Some(ids) => ids
                .iter()
                .filter(|id| usable(id))
                .map(|id| voice_name(id, suffix))
                .collect(),
            None if usable(character) => vec![voice_name(character, suffix)],
            None => Vec::new(),
        };
    };
    let exists = |name: &str| (stems.lookup)(name).is_some();
    if let Some(ids) = list {
        let names: Vec<String> = ids
            .iter()
            .filter(|id| usable(id))
            .map(|id| voice_name(id, suffix))
            .collect();
        if names.len() == ids.len() && names.iter().all(|name| exists(name)) {
            return names;
        }
    }
    if !usable(character) {
        return Vec::new();
    }
    let search = |mut id: &str, first_suffix: &str| {
        let mut name = voice_name(id, first_suffix);
        while !exists(&name) {
            id = shorten(id);
            if id.is_empty() {
                return None;
            }
            name = voice_name(id, suffix);
        }
        Some(name)
    };
    search(character, suffix)
        .or_else(|| search(character, ""))
        .into_iter()
        .collect()
}

/// The metadata `offsets` as the game reads them for the instrumental `inst_id` (`SongOffsets.getInstrumentalOffset(id)` and
/// `getVocalOffset(character, id)`, SongData.hx): an empty id (= null) reads `instrumental` / `vocals`, else `altInstrumentals[id]`
/// (falling back to `instrumental`) / `altVocals[id][character]`.
struct StemOffsets<'a> {
    offsets: Option<&'a Offsets>,
    inst_id: &'a str,
}

fn us(ms: f64, what: &str) -> RoxResult<i64> {
    ms_to_us(ms).ok_or_else(|| bad(format!("the {what} offset ({ms} ms) is out of range")))
}

impl StemOffsets<'_> {
    fn instrumental_us(&self) -> RoxResult<i64> {
        let alt = (!self.inst_id.is_empty())
            .then(|| {
                self.offsets
                    .and_then(|o| o.alt_instrumentals.as_ref())
                    .and_then(|m| m.get(self.inst_id))
            })
            .flatten();
        let ms = alt
            .copied()
            .or_else(|| self.offsets.and_then(|o| o.instrumental))
            .unwrap_or(0.0);
        us(ms, "instrumental")
    }

    fn vocals_us(&self, character: &str, what: &str) -> RoxResult<i64> {
        let found = if self.inst_id.is_empty() {
            self.offsets
                .and_then(|o| o.vocals.as_ref())
                .and_then(|m| m.get(character))
        } else {
            self.offsets
                .and_then(|o| o.alt_vocals.as_ref())
                .and_then(|m| m.get(self.inst_id))
                .and_then(|m| m.get(character))
        };
        us(found.copied().unwrap_or(0.0), what)
    }
}

pub(super) fn build_plan(
    meta: &MetadataFile,
    variation: &str,
    stems: Option<&Stems<'_>>,
    refused: &mut usize,
) -> RoxResult<AudioPlan> {
    if variation != "default" && !component(variation) && !variation.is_empty() {
        return Err(bad(format!("variation {variation:?} is not a file-name part")));
    }
    let suffix = suffix(variation);
    let characters = meta.play_data.as_ref().and_then(|p| p.characters.as_ref());
    let inst_id = characters.and_then(|c| c.instrumental.as_deref()).unwrap_or("");
    let player = characters.and_then(|c| c.player.as_deref()).unwrap_or("");
    let opponent = characters.and_then(|c| c.opponent.as_deref()).unwrap_or("");

    let offsets = StemOffsets { offsets: meta.offsets.as_ref(), inst_id };
    let inst_start = offsets.instrumental_us()?;
    let inst_name = instrumental_name(inst_id, refused)?;
    let player_files = voice_files(characters.and_then(|c| c.player_vocals.as_deref()), player, &suffix, stems, refused);
    let opponent_files = voice_files(characters.and_then(|c| c.opponent_vocals.as_deref()), opponent, &suffix, stems, refused);

    let mut plan = Plan { events: Vec::new(), stems, refused };
    plan.push(inst_start, &inst_name, false)?;
    // Voices are shifted ON TOP of the instrumental: `vocals.time = position - instrumentalOffset` (PlayState.hx), then
    // `voice.time -= voicesOffset` (VoicesGroup.hx): the stem starts at instrumental + vocal offset on the chart's axis.
    let player_start = inst_start + offsets.vocals_us(player, "player vocals")?;
    let opponent_start = inst_start + offsets.vocals_us(opponent, "opponent vocals")?;
    for name in &player_files {
        plan.push(player_start, name, true)?;
    }
    for name in &opponent_files {
        plan.push(opponent_start, name, true)?;
    }
    // `Voices[-<variation>].ogg` is the game's last resort when no character vocals were found; only knowable with the files.
    if let Some(stems) = stems
        && player_files.is_empty()
        && opponent_files.is_empty()
    {
        let legacy = format!("Voices{suffix}.ogg");
        if (stems.lookup)(&legacy).is_some() {
            plan.push(player_start, &legacy, true)?;
        }
    }
    Ok(AudioPlan::new(plan.events))
}

/// The instrumental is mandatory: `Inst.ogg`, or `Inst-<id>.ogg` for `characters.instrumental`.
fn instrumental_name(inst_id: &str, refused: &mut usize) -> RoxResult<String> {
    if inst_id.is_empty() {
        return Ok("Inst.ogg".to_string());
    }
    if !component(inst_id) {
        *refused += 1;
        return Err(bad(format!("instrumental id {inst_id:?} is not a file-name part")));
    }
    Ok(format!("Inst-{inst_id}.ogg"))
}

/// The events of the plan, each stem once.
struct Plan<'a, 's> {
    events: Vec<SoundEvent>,
    stems: Option<&'a Stems<'s>>,
    refused: &'a mut usize,
}

impl Plan<'_, '_> {
    /// Adds the stem `name` at `time_us`; a stem already there is skipped; a missing optional stem of a package is skipped, a
    /// missing instrumental is an error.
    fn push(&mut self, time_us: i64, name: &str, optional: bool) -> RoxResult<()> {
        let Some(file) = safe_name(name, self.refused) else { return Ok(()) };
        if self.events.iter().any(|event| event.index.is_none() && event.file == file) {
            return Ok(());
        }
        let event = match self.stems.and_then(|s| s.package.map(|package| (s, package))) {
            Some((stems, package)) => match (stems.lookup)(&file) {
                Some(index) if self.events.iter().any(|e| e.index == Some(index)) => return Ok(()),
                Some(index) => {
                    let mut event = SoundEvent::in_container(time_us, package, index);
                    event.optional = optional;
                    event
                }
                None if optional => return Ok(()),
                None => return Err(bad(format!("the package has no {file}"))),
            },
            None if optional => SoundEvent::optional_whole(time_us, file),
            None => SoundEvent::whole(time_us, file),
        };
        self.events.push(event);
        Ok(())
    }
}
