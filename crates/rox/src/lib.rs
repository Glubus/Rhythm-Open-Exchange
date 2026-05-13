#![cfg_attr(not(feature = "std"), no_std)]
#![warn(clippy::pedantic)]

#[cfg(all(not(feature = "std"), feature = "alloc"))]
extern crate alloc;

pub mod codec;
pub mod error;
pub mod model;
pub mod prelude;

#[cfg(feature = "std")]
pub use codec::convert_file;
pub use codec::{Decoder, Encoder, Format, convert};
pub use error::{RoxError, RoxResult};
pub use model::{Hitsound, Metadata, Note, NoteType, RoxChart, TimingPoint};
