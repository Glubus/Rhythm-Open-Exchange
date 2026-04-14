#![cfg_attr(not(feature = "std"), no_std)]
#![warn(clippy::pedantic)]

#[cfg(all(not(feature = "std"), feature = "alloc"))]
extern crate alloc;

pub mod fnf;
pub mod jrox;
pub mod osu;
pub mod rox_native;
pub mod sm;
pub mod taiko;

#[cfg(feature = "std")]
pub mod auto;
#[cfg(feature = "std")]
pub mod qua;
#[cfg(feature = "std")]
pub mod yrox;

pub use fnf::{FnfDecoder, FnfEncoder, FnfSide};
pub use jrox::{JroxDecoder, JroxEncoder};
pub use osu::{OsuDecodeOptions, OsuDecoder, OsuEncoder};
pub use rox_native::RoxNativeCodec;
pub use sm::{SmDecoder, SmEncoder};
pub use taiko::TaikoDecoder;

#[cfg(feature = "std")]
pub use auto::{auto_convert, auto_decode, auto_encode};
#[cfg(feature = "std")]
pub use qua::{QuaDecoder, QuaEncoder};
#[cfg(feature = "std")]
pub use yrox::{YroxDecoder, YroxEncoder};
