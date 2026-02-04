use crate::error::{RoxError, RoxResult};
use std::path::Path;

/// Supported input format extensions for decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    /// Native ROX binary format (`.rox`)
    #[cfg(feature = "compression")]
    Rox,
    /// JSON ROX format (`.jrox`)
    Jrox,
    /// YAML ROX format (`.yrox`)
    Yrox,
    /// osu!mania format (`.osu`)
    Osu,
    /// osu!taiko format (`.osu` with mode detection)
    Taiko,
    /// `StepMania` format (`.sm`)
    Sm,
    /// Quaver format (`.qua`)
    Qua,
    /// Friday Night Funkin' format (`.json`)
    Fnf,
}

/// Supported output format extensions for encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Native ROX binary format (`.rox`)
    #[cfg(feature = "compression")]
    Rox,
    /// JSON ROX format (`.jrox`)
    Jrox,
    /// YAML ROX format (`.yrox`)
    Yrox,
    /// osu!mania format (`.osu`)
    Osu,
    /// `StepMania` format (`.sm`)
    Sm,
    /// Quaver format (`.qua`)
    Qua,
    /// Friday Night Funkin' format (`.json`)
    Fnf,
}

impl InputFormat {
    /// All supported input extensions.
    pub const EXTENSIONS: &'static [(&'static str, Self)] = &[
        #[cfg(feature = "compression")]
        ("rox", Self::Rox),
        ("jrox", Self::Jrox),
        ("yrox", Self::Yrox),
        ("osu", Self::Osu),
        ("sm", Self::Sm),
        ("qua", Self::Qua),
        ("json", Self::Fnf),
    ];

    /// Detect format from file extension.
    ///
    /// # Errors
    ///
    /// Returns an error if the extension is not recognized.
    pub fn from_extension(ext: &str) -> RoxResult<Self> {
        let ext_lower = ext.to_lowercase();
        for (e, format) in Self::EXTENSIONS {
            if *e == ext_lower {
                return Ok(*format);
            }
        }
        Err(RoxError::UnsupportedFormat(format!(
            "Unknown input extension: .{ext}"
        )))
    }

    /// Detect format from file path.
    ///
    /// # Errors
    ///
    /// Returns an error if the path has no extension or it's not recognized.
    pub fn from_path(path: impl AsRef<Path>) -> RoxResult<Self> {
        let path = path.as_ref();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| RoxError::InvalidFormat("No file extension".into()))?;
        Self::from_extension(ext)
    }
}

impl OutputFormat {
    /// All supported output extensions.
    pub const EXTENSIONS: &'static [(&'static str, Self)] = &[
        #[cfg(feature = "compression")]
        ("rox", Self::Rox),
        ("jrox", Self::Jrox),
        ("yrox", Self::Yrox),
        ("osu", Self::Osu),
        ("sm", Self::Sm),
        ("qua", Self::Qua),
        ("json", Self::Fnf),
    ];

    /// Detect format from file extension.
    ///
    /// # Errors
    ///
    /// Returns an error if the extension is not recognized.
    pub fn from_extension(ext: &str) -> RoxResult<Self> {
        let ext_lower = ext.to_lowercase();
        for (e, format) in Self::EXTENSIONS {
            if *e == ext_lower {
                return Ok(*format);
            }
        }
        Err(RoxError::UnsupportedFormat(format!(
            "Unknown output extension: .{ext}"
        )))
    }

    /// Detect format from file path.
    ///
    /// # Errors
    ///
    /// Returns an error if the path has no extension or it's not recognized.
    pub fn from_path(path: impl AsRef<Path>) -> RoxResult<Self> {
        let path = path.as_ref();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| RoxError::InvalidFormat("No file extension".into()))?;
        Self::from_extension(ext)
    }
}
