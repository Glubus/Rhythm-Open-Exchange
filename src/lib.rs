#![cfg_attr(not(feature = "std"), no_std)]
#![warn(clippy::pedantic)]

// Allows the former workspace modules to keep their stable `rox::…` paths
// while living in this single package.
extern crate self as rox;
#[cfg(test)]
extern crate self as rox_test_utils;

#[cfg(not(feature = "std"))]
extern crate alloc;

/// Implements [`codec::Format`] for a codec and its recognized extensions.
///
/// Kept as a declarative macro so format implementations remain in this
/// package; a proc-macro crate would require a second package.
#[macro_export]
macro_rules! impl_format {
    ($type:ty, [$($extension:literal),* $(,)?]) => {
        impl $crate::codec::Format for $type {
            const EXTENSIONS: &'static [&'static str] = &[$($extension),*];
        }
    };
}

pub mod codec;
pub mod error;
pub mod model;
pub mod prelude;

#[cfg(feature = "formats")]
pub mod formats;

#[cfg(feature = "analysis")]
pub mod analysis;

#[cfg(test)]
mod test_utils;
#[cfg(test)]
pub use test_utils::get_test_asset;

#[cfg(feature = "std")]
pub use codec::convert_file;
pub use codec::{Decoder, Encoder, Format, convert};
pub use error::{RoxError, RoxResult};
pub use model::{Hitsound, Metadata, Note, NoteType, RoxChart, TimingPoint};
