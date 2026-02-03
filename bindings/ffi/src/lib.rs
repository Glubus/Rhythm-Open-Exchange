use rhythm_open_exchange::analysis::pattern_recognition::AnalysisResult as InternalAnalysisResult;
use rhythm_open_exchange::error::RoxError;
use rhythm_open_exchange::model::{
    Note as InternalNote, NoteType, RoxChart as InternalChart, TimingPoint as InternalTimingPoint,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

uniffi::setup_scaffolding!("rox_ffi");

#[derive(Debug, uniffi::Error)]
pub enum FfiError {
    Generic { message: String },
}

impl From<RoxError> for FfiError {
    fn from(e: RoxError) -> Self {
        Self::Generic {
            message: e.to_string(),
        }
    }
}

impl std::fmt::Display for FfiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FfiError::Generic { message } => write!(f, "{}", message),
        }
    }
}

/// Type of note exposed to FFI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiNoteType {
    Tap,
    Hold,
    Burst,
    Mine,
}

/// A single note exposed to FFI.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiNote {
    pub time_us: i64,
    pub column: u8,
    pub duration_us: i64,
    pub note_type: FfiNoteType,
}

impl From<&InternalNote> for FfiNote {
    fn from(n: &InternalNote) -> Self {
        let (note_type, duration_us) = match n.note_type {
            NoteType::Tap => (FfiNoteType::Tap, 0),
            NoteType::Hold { duration_us } => (FfiNoteType::Hold, duration_us),
            NoteType::Burst { duration_us } => (FfiNoteType::Burst, duration_us),
            NoteType::Mine => (FfiNoteType::Mine, 0),
        };
        Self {
            time_us: n.time_us,
            column: n.column,
            duration_us,
            note_type,
        }
    }
}

/// A timing point exposed to FFI.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiTimingPoint {
    pub time_us: i64,
    pub bpm: f32,
    pub signature: u8,
    pub is_inherited: bool,
    pub scroll_speed: f32,
}

impl From<&InternalTimingPoint> for FfiTimingPoint {
    fn from(tp: &InternalTimingPoint) -> Self {
        Self {
            time_us: tp.time_us,
            bpm: tp.bpm,
            signature: tp.signature,
            is_inherited: tp.is_inherited,
            scroll_speed: tp.scroll_speed,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiPatternEntry {
    pub time_start_us: i64,
    pub time_end_us: i64,
    pub pattern: String,
    pub note_count: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FfiAnalysisResult {
    pub timeline: Vec<FfiPatternEntry>,
    pub key_count: u8,
}

impl From<InternalAnalysisResult> for FfiAnalysisResult {
    fn from(res: InternalAnalysisResult) -> Self {
        Self {
            timeline: res
                .timeline
                .entries
                .into_iter()
                .map(|e| FfiPatternEntry {
                    time_start_us: e.start_time,
                    time_end_us: e.end_time,
                    pattern: e.pattern_type.as_str().to_string(), // Convert enum to string for FFI
                    note_count: e.note_count as u64,
                })
                .collect(),
            key_count: res.key_count,
        }
    }
}

#[derive(uniffi::Object)]
pub struct RoxChart {
    // We use RwLock to allow mutation via FFI (interior mutability)
    inner: RwLock<InternalChart>,
}

#[uniffi::export]
impl RoxChart {
    #[uniffi::constructor]
    pub fn new(key_count: u8) -> Self {
        Self {
            inner: RwLock::new(InternalChart::new(key_count)),
        }
    }

    // --- Metadata ---

    pub fn title(&self) -> String {
        self.inner.read().unwrap().metadata.title.to_string()
    }

    pub fn set_title(&self, title: String) {
        self.inner.write().unwrap().metadata.title = title.into();
    }

    pub fn artist(&self) -> String {
        self.inner.read().unwrap().metadata.artist.to_string()
    }

    pub fn set_artist(&self, artist: String) {
        self.inner.write().unwrap().metadata.artist = artist.into();
    }

    pub fn creator(&self) -> String {
        self.inner.read().unwrap().metadata.creator.to_string()
    }

    pub fn set_creator(&self, creator: String) {
        self.inner.write().unwrap().metadata.creator = creator.into();
    }

    pub fn difficulty(&self) -> String {
        self.inner
            .read()
            .unwrap()
            .metadata
            .difficulty_name
            .to_string()
    }

    pub fn set_difficulty(&self, difficulty: String) {
        self.inner.write().unwrap().metadata.difficulty_name = difficulty.into();
    }

    pub fn audio_file(&self) -> String {
        self.inner.read().unwrap().metadata.audio_file.to_string()
    }

    pub fn set_audio_file(&self, audio_file: String) {
        self.inner.write().unwrap().metadata.audio_file = audio_file.into();
    }

    pub fn key_count(&self) -> u8 {
        self.inner.read().unwrap().key_count()
    }

    pub fn is_coop(&self) -> bool {
        self.inner.read().unwrap().metadata.is_coop
    }

    pub fn set_coop(&self, is_coop: bool) {
        self.inner.write().unwrap().metadata.is_coop = is_coop;
    }

    pub fn offset(&self) -> i64 {
        self.inner.read().unwrap().metadata.audio_offset_us
    }

    pub fn set_offset(&self, offset_us: i64) {
        self.inner.write().unwrap().metadata.audio_offset_us = offset_us;
    }

    // --- Stats ---

    pub fn duration_seconds(&self) -> f64 {
        self.inner.read().unwrap().duration_us() as f64 / 1_000_000.0
    }

    pub fn note_count(&self) -> u64 {
        self.inner.read().unwrap().note_count() as u64
    }

    pub fn hash(&self) -> String {
        // Requires 'analysis' feature
        use rhythm_open_exchange::analysis::RoxAnalysis;
        self.inner.read().unwrap().hash()
    }

    pub fn notes_hash(&self) -> String {
        use rhythm_open_exchange::analysis::RoxAnalysis;
        self.inner.read().unwrap().notes_hash()
    }

    pub fn timings_hash(&self) -> String {
        use rhythm_open_exchange::analysis::RoxAnalysis;
        self.inner.read().unwrap().timings_hash()
    }

    pub fn short_hash(&self) -> String {
        use rhythm_open_exchange::analysis::RoxAnalysis;
        self.inner.read().unwrap().short_hash()
    }

    pub fn bpm_min(&self) -> f64 {
        use rhythm_open_exchange::analysis::RoxAnalysis;
        self.inner.read().unwrap().bpm_min()
    }

    pub fn bpm_max(&self) -> f64 {
        use rhythm_open_exchange::analysis::RoxAnalysis;
        self.inner.read().unwrap().bpm_max()
    }

    pub fn bpm_mode(&self) -> f64 {
        use rhythm_open_exchange::analysis::RoxAnalysis;
        self.inner.read().unwrap().bpm_mode()
    }

    pub fn nps(&self) -> f64 {
        use rhythm_open_exchange::analysis::RoxAnalysis;
        self.inner.read().unwrap().nps()
    }

    pub fn highest_nps(&self, window_size_s: f64) -> f64 {
        use rhythm_open_exchange::analysis::RoxAnalysis;
        self.inner.read().unwrap().highest_nps(window_size_s)
    }

    pub fn lowest_nps(&self, window_size_s: f64) -> f64 {
        use rhythm_open_exchange::analysis::RoxAnalysis;
        self.inner.read().unwrap().lowest_nps(window_size_s)
    }

    pub fn highest_drain_time(&self) -> f64 {
        use rhythm_open_exchange::analysis::RoxAnalysis;
        self.inner.read().unwrap().highest_drain_time()
    }

    pub fn density(&self, segments: u64) -> Vec<f64> {
        use rhythm_open_exchange::analysis::RoxAnalysis;
        // segments is usize in the trait, but usually passed as u64/i64 in FFI.
        // UniFFI supports u64. Cast to usize.
        self.inner.read().unwrap().density(segments as usize)
    }

    pub fn polyphony(&self) -> HashMap<u32, u32> {
        use rhythm_open_exchange::analysis::RoxAnalysis;
        self.inner.read().unwrap().polyphony()
    }

    pub fn lane_balance(&self) -> Vec<u32> {
        use rhythm_open_exchange::analysis::RoxAnalysis;
        self.inner.read().unwrap().lane_balance()
    }

    pub fn analyze_patterns(&self) -> FfiAnalysisResult {
        use rhythm_open_exchange::analysis::RoxAnalysis;
        self.inner.read().unwrap().pattern_analysis().into()
    }

    // --- Notes Manipulation ---

    pub fn add_tap(&self, time_us: i64, column: u8) {
        self.inner
            .write()
            .unwrap()
            .notes
            .push(InternalNote::tap(time_us, column));
    }

    pub fn add_hold(&self, time_us: i64, duration_us: i64, column: u8) {
        self.inner
            .write()
            .unwrap()
            .notes
            .push(InternalNote::hold(time_us, duration_us, column));
    }

    pub fn add_burst(&self, time_us: i64, duration_us: i64, column: u8) {
        self.inner
            .write()
            .unwrap()
            .notes
            .push(InternalNote::burst(time_us, duration_us, column));
    }

    pub fn add_mine(&self, time_us: i64, column: u8) {
        self.inner
            .write()
            .unwrap()
            .notes
            .push(InternalNote::mine(time_us, column));
    }

    pub fn clear_notes(&self) {
        self.inner.write().unwrap().notes.clear();
    }

    pub fn get_notes(&self) -> Vec<FfiNote> {
        self.inner
            .read()
            .unwrap()
            .notes
            .iter()
            .map(FfiNote::from)
            .collect()
    }

    // --- Timing Points Manipulation ---

    pub fn add_bpm(&self, time_us: i64, bpm: f32) {
        self.inner
            .write()
            .unwrap()
            .timing_points
            .push(InternalTimingPoint::bpm(time_us, bpm));
    }

    pub fn add_sv(&self, time_us: i64, scroll_speed: f32) {
        self.inner
            .write()
            .unwrap()
            .timing_points
            .push(InternalTimingPoint::sv(time_us, scroll_speed));
    }

    pub fn clear_timing_points(&self) {
        self.inner.write().unwrap().timing_points.clear();
    }

    pub fn get_timing_points(&self) -> Vec<FfiTimingPoint> {
        self.inner
            .read()
            .unwrap()
            .timing_points
            .iter()
            .map(FfiTimingPoint::from)
            .collect()
    }

    // --- Analysis/Validation ---

    pub fn validate(&self) -> Result<(), FfiError> {
        self.inner.read().unwrap().validate().map_err(Into::into)
    }
}

// --- Global Functions (Codec) ---

#[uniffi::export]
pub fn decode_chart(path: String) -> Result<Arc<RoxChart>, FfiError> {
    let chart = rhythm_open_exchange::codec::auto_decode(&path)?;
    Ok(Arc::new(RoxChart {
        inner: RwLock::new(chart),
    }))
}

#[uniffi::export]
pub fn decode_from_bytes(data: Vec<u8>) -> Result<Arc<RoxChart>, FfiError> {
    let chart = rhythm_open_exchange::codec::from_bytes(&data)?;
    Ok(Arc::new(RoxChart {
        inner: RwLock::new(chart),
    }))
}

#[uniffi::export]
pub fn decode_from_string(data: String) -> Result<Arc<RoxChart>, FfiError> {
    let chart = rhythm_open_exchange::codec::from_string(&data)?;
    Ok(Arc::new(RoxChart {
        inner: RwLock::new(chart),
    }))
}

#[uniffi::export]
pub fn encode_chart(chart: &RoxChart, path: String) -> Result<(), FfiError> {
    rhythm_open_exchange::codec::auto_encode(&chart.inner.read().unwrap(), &path)
        .map_err(Into::into)
}

#[uniffi::export]
pub fn auto_convert(input: String, output: String) -> Result<(), FfiError> {
    rhythm_open_exchange::codec::auto_convert(&input, &output).map_err(Into::into)
}
