#![cfg_attr(not(feature = "std"), no_std)]
#![warn(clippy::pedantic)]

#[cfg(all(not(feature = "std"), feature = "alloc"))]
extern crate alloc;

pub mod error;

pub use error::{RoxError, RoxResult};
