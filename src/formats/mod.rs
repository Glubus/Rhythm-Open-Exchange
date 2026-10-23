#![cfg_attr(not(feature = "std"), no_std)]
#![warn(clippy::pedantic)]

#[cfg(all(not(feature = "std"), feature = "alloc"))]
extern crate alloc;

pub mod fnf;
pub mod jrox;
pub mod mc;
pub mod osu;
pub mod rox_native;
pub mod sm;
pub mod taiko;

#[cfg(feature = "std")]
pub mod auto;
#[cfg(feature = "std")]
pub mod names;
#[cfg(feature = "std")]
pub mod qua;
#[cfg(feature = "std")]
pub mod yrox;

pub use fnf::{FnfDecoder, FnfEncoder, FnfSide};
pub use jrox::{JroxDecoder, JroxEncoder};
pub use mc::{McDecoder, McEncoder};
pub use osu::{OsuDecodeOptions, OsuDecoder, OsuEncoder};
pub use rox_native::RoxNativeCodec;
pub use sm::{SmDecoder, SmEncoder, SscDecoder, SscEncoder};
pub use taiko::TaikoDecoder;

#[cfg(feature = "std")]
pub use auto::{auto_convert, auto_decode, auto_decode_bytes, auto_decode_variant, auto_encode};
#[cfg(feature = "std")]
pub use names::{RomanisedNames, romanised_names, romanised_names_from_bytes};
#[cfg(feature = "std")]
pub use qua::{QuaDecoder, QuaEncoder};
#[cfg(feature = "std")]
pub use yrox::{YroxDecoder, YroxEncoder};
