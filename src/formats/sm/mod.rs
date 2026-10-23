pub mod decoder;
pub mod encoder;
pub mod msd;
pub mod parser;
#[cfg(all(test, feature = "std"))]
mod sm_tests;
pub mod ssc_decoder;
pub mod ssc_encoder;
pub mod ssc_parser;
#[cfg(all(test, feature = "std"))]
mod ssc_tests;
pub mod timing_data;
pub mod types;
pub use decoder::SmDecoder;
pub use encoder::SmEncoder;
pub use ssc_decoder::SscDecoder;
pub use ssc_encoder::SscEncoder;

/// Whether `path` is a StepMania/Etterna chart file (`.sm` or `.ssc`, any case).
#[cfg(feature = "std")]
#[must_use]
pub fn is_stepmania(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("sm") || ext.eq_ignore_ascii_case("ssc"))
}

/// Reads and parses a `.sm` or `.ssc` file with the parser of its extension; every playable chart
/// is in `charts`, in file order (their indices are the file's variants).
///
/// Like StepMania, a file whose `#MUSIC` is empty or missing from its folder plays the first
/// audio file (`.ogg`, `.oga`, `.mp3`, `.wav`, `.flac`, by name) of that folder.
///
/// # Errors
/// Returns an error if the file can not be read or parsed, or is not a StepMania file.
#[cfg(feature = "std")]
pub fn parse_file(path: &std::path::Path) -> rox::error::RoxResult<types::SmFile> {
    let data = std::fs::read(path)?;
    parse_bytes(path, &data)
}

/// [`parse_file`] for a file whose bytes the caller already read (`path` names its extension and its folder, which the
/// `#MUSIC` fallback looks into): a scan reads each file once and hands the same bytes to every reader.
///
/// # Errors
/// Returns an error if the data can not be parsed, or `path` is not a StepMania file.
#[cfg(feature = "std")]
pub fn parse_bytes(path: &std::path::Path, data: &[u8]) -> rox::error::RoxResult<types::SmFile> {
    let mut file = match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("sm") => parser::parse(data)?,
        Some("ssc") => ssc_parser::parse(data)?,
        _ => {
            return Err(rox::error::RoxError::UnsupportedFormat(
                path.display().to_string(),
            ));
        }
    };
    let folder = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    if !file.metadata.music.is_empty() && folder.join(&file.metadata.music).is_file() {
        return Ok(file);
    }
    if let Some(music) = first_audio_file(folder) {
        file.metadata.music = music;
    }
    Ok(file)
}

/// The first audio file of `folder` by name, ignoring case.
#[cfg(feature = "std")]
fn first_audio_file(folder: &std::path::Path) -> Option<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .ok()?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| {
                    ["ogg", "oga", "mp3", "wav", "flac"]
                        .iter()
                        .any(|audio| ext.eq_ignore_ascii_case(audio))
                })
        })
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names.into_iter().next()
}
