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
pub mod bms;
#[cfg(feature = "std")]
pub mod clone_hero;
#[cfg(feature = "std")]
pub mod dwi;
#[cfg(feature = "std")]
pub mod fnf_vslice;
#[cfg(feature = "std")]
pub mod ksf;
#[cfg(feature = "std")]
pub mod names;
#[cfg(feature = "std")]
pub mod o2jam;
#[cfg(feature = "std")]
pub mod qua;
#[cfg(feature = "std")]
pub mod sm_edit;
#[cfg(feature = "std")]
pub mod ssq;
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
pub use auto::{
    FileVariant, auto_convert, auto_decode, auto_decode_all, auto_decode_bytes, auto_decode_variant,
    auto_encode,
};
#[cfg(feature = "std")]
pub use bms::{BmsDecoder, BmsonDecoder, PmsDecoder};
#[cfg(feature = "std")]
pub use dwi::DwiDecoder;
#[cfg(feature = "std")]
pub use clone_hero::{ChartDecoder, SngPackage, SongIni};
#[cfg(feature = "std")]
pub use ksf::{KsfContext, KsfDecoder};
#[cfg(feature = "std")]
pub use names::{RomanisedNames, romanised_names, romanised_names_from_bytes};
#[cfg(feature = "std")]
pub use o2jam::{OjmContainer, OjnDecoder};
#[cfg(feature = "std")]
pub use fnf_vslice::{FnfcPackage, VSliceDecoder, is_vslice_chart_name};
#[cfg(feature = "std")]
pub use qua::{QuaDecoder, QuaEncoder};
#[cfg(feature = "std")]
pub use sm_edit::EditDecoder;
#[cfg(feature = "std")]
pub use ssq::SsqDecoder;
#[cfg(feature = "std")]
pub use yrox::{YroxDecoder, YroxEncoder};
