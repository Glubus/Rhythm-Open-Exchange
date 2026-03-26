#![cfg_attr(not(feature = "std"), no_std)]
#![warn(clippy::pedantic)]

#[cfg(all(not(feature = "std"), feature = "alloc"))]
extern crate alloc;

pub mod codec;
pub mod error;
pub mod model;

pub use error::{RoxError, RoxResult};
pub use model::{Hitsound, Metadata, Note, NoteType, RoxChart, TimingPoint};
