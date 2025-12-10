//! WebAssembly bindings for Rhythm Open Exchange.
//!
//! Provides JavaScript/TypeScript access to chart decoding, encoding, and conversion.

use rhythm_open_exchange::codec::formats::{
    FnfDecoder, FnfEncoder, OsuDecoder, OsuEncoder, QuaDecoder, QuaEncoder, SmDecoder, SmEncoder,
};
use rhythm_open_exchange::codec::rox::RoxCodec;
use rhythm_open_exchange::codec::{Decoder, Encoder};
use rhythm_open_exchange::model::RoxChart;
use wasm_bindgen::prelude::*;

/// A rhythm game chart (WASM wrapper).
#[wasm_bindgen]
pub struct Chart {
    inner: RoxChart,
}

#[wasm_bindgen]
impl Chart {
    /// Chart title.
    #[wasm_bindgen(getter)]
    pub fn title(&self) -> String {
        self.inner.metadata.title.clone()
    }

    /// Chart artist.
    #[wasm_bindgen(getter)]
    pub fn artist(&self) -> String {
        self.inner.metadata.artist.clone()
    }

    /// Chart creator/mapper.
    #[wasm_bindgen(getter)]
    pub fn creator(&self) -> String {
        self.inner.metadata.creator.clone()
    }

    /// Difficulty name.
    #[wasm_bindgen(getter)]
    pub fn difficulty(&self) -> String {
        self.inner.metadata.difficulty_name.clone()
    }

    /// Number of keys/columns.
    #[wasm_bindgen(getter)]
    pub fn key_count(&self) -> u8 {
        self.inner.key_count
    }

    /// Number of notes.
    #[wasm_bindgen(getter)]
    pub fn note_count(&self) -> usize {
        self.inner.notes.len()
    }

    /// Duration in seconds.
    #[wasm_bindgen(getter)]
    pub fn duration(&self) -> f64 {
        self.inner.duration_us() as f64 / 1_000_000.0
    }

    /// Whether this is a coop chart.
    #[wasm_bindgen(getter)]
    pub fn is_coop(&self) -> bool {
        self.inner.metadata.is_coop
    }

    /// Short hash of the chart.
    #[wasm_bindgen(getter)]
    pub fn hash(&self) -> String {
        self.inner.short_hash()
    }

    /// Audio file path.
    #[wasm_bindgen(getter)]
    pub fn audio_file(&self) -> String {
        self.inner.metadata.audio_file.clone()
    }
}

/// Decode chart bytes with the specified format.
///
/// Formats: "rox", "osu", "sm", "qua", "json"/"fnf"
#[wasm_bindgen]
pub fn decode(data: &[u8], format: &str) -> Result<Chart, JsError> {
    let chart = match format.to_lowercase().as_str() {
        "rox" => RoxCodec::decode(data),
        "osu" => OsuDecoder::decode(data),
        "sm" => SmDecoder::decode(data),
        "qua" => QuaDecoder::decode(data),
        "json" | "fnf" => FnfDecoder::decode(data),
        _ => return Err(JsError::new(&format!("Unknown format: {format}"))),
    };
    chart
        .map(|inner| Chart { inner })
        .map_err(|e| JsError::new(&format!("Decode error: {e}")))
}

/// Encode a chart to bytes with the specified format.
///
/// Formats: "rox", "osu", "sm", "qua", "json"/"fnf"
#[wasm_bindgen]
pub fn encode(chart: &Chart, format: &str) -> Result<Vec<u8>, JsError> {
    let result = match format.to_lowercase().as_str() {
        "rox" => RoxCodec::encode(&chart.inner),
        "osu" => OsuEncoder::encode(&chart.inner),
        "sm" => SmEncoder::encode(&chart.inner),
        "qua" => QuaEncoder::encode(&chart.inner),
        "json" | "fnf" => FnfEncoder::encode(&chart.inner),
        _ => return Err(JsError::new(&format!("Unknown format: {format}"))),
    };
    result.map_err(|e| JsError::new(&format!("Encode error: {e}")))
}

/// Convert chart bytes from one format to another.
#[wasm_bindgen]
pub fn convert(data: &[u8], from_format: &str, to_format: &str) -> Result<Vec<u8>, JsError> {
    let chart = decode(data, from_format)?;
    encode(&chart, to_format)
}

/// Get library version.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
