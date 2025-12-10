//! Python bindings for Rhythm Open Exchange.
//!
//! Provides Python access to chart decoding, encoding, and conversion.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use rhythm_open_exchange::codec::rox::RoxCodec;
use rhythm_open_exchange::codec::{Decoder, Encoder, auto_decode, auto_encode};
use rhythm_open_exchange::model::{Note, RoxChart, TimingPoint};

/// A rhythm game chart.
#[pyclass]
#[derive(Clone)]
pub struct PyChart {
    inner: RoxChart,
}

#[pymethods]
impl PyChart {
    /// Chart title.
    #[getter]
    fn title(&self) -> String {
        self.inner.metadata.title.clone()
    }

    /// Chart artist.
    #[getter]
    fn artist(&self) -> String {
        self.inner.metadata.artist.clone()
    }

    /// Chart creator/mapper.
    #[getter]
    fn creator(&self) -> String {
        self.inner.metadata.creator.clone()
    }

    /// Difficulty name.
    #[getter]
    fn difficulty(&self) -> String {
        self.inner.metadata.difficulty_name.clone()
    }

    /// Number of keys/columns.
    #[getter]
    fn key_count(&self) -> u8 {
        self.inner.key_count
    }

    /// Number of notes.
    #[getter]
    fn note_count(&self) -> usize {
        self.inner.notes.len()
    }

    /// Duration in seconds.
    #[getter]
    fn duration(&self) -> f64 {
        self.inner.duration_us() as f64 / 1_000_000.0
    }

    /// Whether this is a coop chart.
    #[getter]
    fn is_coop(&self) -> bool {
        self.inner.metadata.is_coop
    }

    /// Short hash of the chart.
    #[getter]
    fn hash(&self) -> String {
        self.inner.short_hash()
    }

    /// String representation.
    fn __repr__(&self) -> String {
        format!(
            "Chart({} - {} [{}K, {} notes])",
            self.artist(),
            self.title(),
            self.key_count(),
            self.note_count()
        )
    }
}

/// Decode a chart file from path.
///
/// Supports: .rox, .osu, .sm, .qua, .json (FNF)
#[pyfunction]
fn decode(path: &str) -> PyResult<PyChart> {
    auto_decode(path)
        .map(|inner| PyChart { inner })
        .map_err(|e| PyValueError::new_err(format!("Decode error: {e}")))
}

/// Decode a chart from bytes with explicit format.
#[pyfunction]
fn decode_bytes(data: &[u8], format: &str) -> PyResult<PyChart> {
    let chart = match format.to_lowercase().as_str() {
        "rox" => RoxCodec::decode(data),
        "osu" => rhythm_open_exchange::codec::formats::OsuDecoder::decode(data),
        "sm" => rhythm_open_exchange::codec::formats::SmDecoder::decode(data),
        "qua" => rhythm_open_exchange::codec::formats::QuaDecoder::decode(data),
        "json" | "fnf" => rhythm_open_exchange::codec::formats::FnfDecoder::decode(data),
        _ => return Err(PyValueError::new_err(format!("Unknown format: {format}"))),
    };
    chart
        .map(|inner| PyChart { inner })
        .map_err(|e| PyValueError::new_err(format!("Decode error: {e}")))
}

/// Encode a chart to a file.
///
/// Format is detected from file extension.
#[pyfunction]
fn encode(chart: &PyChart, path: &str) -> PyResult<()> {
    auto_encode(&chart.inner, path).map_err(|e| PyValueError::new_err(format!("Encode error: {e}")))
}

/// Encode a chart to bytes with explicit format.
#[pyfunction]
fn encode_bytes(chart: &PyChart, format: &str) -> PyResult<Vec<u8>> {
    let result = match format.to_lowercase().as_str() {
        "rox" => RoxCodec::encode(&chart.inner),
        "osu" => rhythm_open_exchange::codec::formats::OsuEncoder::encode(&chart.inner),
        "sm" => rhythm_open_exchange::codec::formats::SmEncoder::encode(&chart.inner),
        "qua" => rhythm_open_exchange::codec::formats::QuaEncoder::encode(&chart.inner),
        "json" | "fnf" => rhythm_open_exchange::codec::formats::FnfEncoder::encode(&chart.inner),
        _ => return Err(PyValueError::new_err(format!("Unknown format: {format}"))),
    };
    result.map_err(|e| PyValueError::new_err(format!("Encode error: {e}")))
}

/// Convert a chart file from one format to another.
#[pyfunction]
fn convert(input: &str, output: &str) -> PyResult<()> {
    let chart =
        auto_decode(input).map_err(|e| PyValueError::new_err(format!("Decode error: {e}")))?;
    auto_encode(&chart, output).map_err(|e| PyValueError::new_err(format!("Encode error: {e}")))
}

/// ROX - Rhythm Open Exchange Python bindings.
#[pymodule]
fn rox(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyChart>()?;
    m.add_function(wrap_pyfunction!(decode, m)?)?;
    m.add_function(wrap_pyfunction!(decode_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(encode, m)?)?;
    m.add_function(wrap_pyfunction!(encode_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(convert, m)?)?;
    Ok(())
}
