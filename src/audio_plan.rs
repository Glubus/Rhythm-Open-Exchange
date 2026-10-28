//! The AUDIO PLAN of a chart: every sound a player would trigger, with its time.
//!
//! Some formats have no single music file. BMS places keysounds and background sounds on a timeline, bmson slices
//! stems, `O2Jam` keeps samples in an `.ojm`. A decoder that can describe this returns an [`AudioPlan`] BESIDE the
//! chart (`decode_with_audio_plan`); a consumer that wants one audio track mixes the plan (Prism renders it to Ogg Opus).
//!
//! The plan is never part of [`crate::RoxChart`]: it is not archived in `.rox` files and does not enter any hash.
//! Times are on the chart's own time axis (the one of `RoxChart::notes`), in microseconds; an event before 0 is
//! before the start of the audio.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use compact_str::CompactString;

/// One sound to play: `file` from `from_us` for `len_us` (to its end when `None`), starting at `time_us`.
///
/// A whole-track stem is an event at the stem's start with `from_us` 0 and `len_us` `None`.
///
/// When `index` is `Some`, `file` names a CONTAINER file (for `O2Jam`, an `.ojm`) and `index` is the id of the sample
/// inside it, as `OjmContainer::sample(id)` takes it; `from_us` and `len_us` then cut that sample. When `index` is
/// `None` (BMS, bmson) `file` is the whole sound file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SoundEvent {
    /// When the sound starts, in µs on the chart's time axis.
    pub time_us: i64,
    /// File name as the chart wrote it, relative to the chart's folder, `/` separated, never absolute and never
    /// holding `..`. Its extension may be wrong (the consumer looks for the same name with another audio extension).
    pub file: CompactString,
    /// Offset into the file where the sound begins, µs.
    pub from_us: i64,
    /// Length played, µs; `None` plays to the end of the file.
    pub len_us: Option<i64>,
    /// Gain in percent (100 = unchanged); `None` = unchanged.
    pub volume: Option<u8>,
    /// Sample id inside the container `file` (see above); `None` = `file` itself is the sound.
    pub index: Option<u32>,
    /// A file that may legitimately be absent (a stem the song does not have, such as `Voices-<id>.ogg` or `rhythm.ogg`):
    /// when the consumer cannot find it, it skips the event WITHOUT counting a missing sound.
    pub optional: bool,
}

impl SoundEvent {
    /// A whole file starting at `time_us`.
    #[must_use]
    pub fn whole(time_us: i64, file: impl Into<CompactString>) -> Self {
        Self {
            time_us,
            file: file.into(),
            from_us: 0,
            len_us: None,
            volume: None,
            index: None,
            optional: false,
        }
    }

    /// A whole file that may be absent, see [`SoundEvent::optional`].
    #[must_use]
    pub fn optional_whole(time_us: i64, file: impl Into<CompactString>) -> Self {
        Self {
            optional: true,
            ..Self::whole(time_us, file)
        }
    }

    /// The whole sample `index` of the container file `container_file`, starting at `time_us`.
    #[must_use]
    pub fn in_container(
        time_us: i64,
        container_file: impl Into<CompactString>,
        index: u32,
    ) -> Self {
        Self {
            index: Some(index),
            ..Self::whole(time_us, container_file)
        }
    }
}

/// The sounds of a chart, sorted by time (events at one time keep the order the decoder produced them).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct AudioPlan {
    pub events: Vec<SoundEvent>,
}

impl AudioPlan {
    /// A plan from `events`, sorted by time (stable).
    #[must_use]
    pub fn new(mut events: Vec<SoundEvent>) -> Self {
        events.sort_by_key(|event| event.time_us);
        Self { events }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// The distinct file names of the plan, sorted.
    #[must_use]
    pub fn files(&self) -> Vec<&str> {
        let mut files: Vec<&str> = self
            .events
            .iter()
            .map(|event| event.file.as_str())
            .collect();
        files.sort_unstable();
        files.dedup();
        files
    }
}
