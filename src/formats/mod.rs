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
pub mod bandori;
#[cfg(feature = "std")]
pub mod bms;
#[cfg(feature = "std")]
pub mod clone_hero;
#[cfg(feature = "std")]
pub mod dtx;
#[cfg(feature = "std")]
pub mod dwi;
#[cfg(feature = "std")]
pub mod fnf_vslice;
#[cfg(feature = "std")]
pub mod imd;
#[cfg(feature = "std")]
pub mod ksf;
#[cfg(feature = "std")]
pub mod ksh;
#[cfg(feature = "std")]
pub mod names;
#[cfg(feature = "std")]
pub mod o2jam;
#[cfg(feature = "std")]
pub mod qua;
#[cfg(feature = "std")]
pub mod sekai;
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
pub use imd::{ImdDecoder, ImdReport, is_imd_chart_file, looks_like_imd};
#[cfg(feature = "std")]
pub use bandori::{
    BandoriReport, BestdoriDecoder, SimulatorDecoder, is_bandori_chart_file, is_bestdori_chart_name,
    looks_like_bestdori, looks_like_simulator,
};
#[cfg(feature = "std")]
pub use bms::{BmsDecoder, BmsonDecoder, PmsDecoder};
#[cfg(feature = "std")]
pub use dtx::{DtxDecoder, DtxReport};
#[cfg(feature = "std")]
pub use dwi::{DwiDecoder, MsdDecoder};
#[cfg(feature = "std")]
pub use clone_hero::{ChartDecoder, MidDecoder, SngPackage, SongIni, is_mid_chart_name};
#[cfg(feature = "std")]
pub use ksf::{KsfContext, KsfDecoder};
#[cfg(feature = "std")]
pub use ksh::{KshDecoder, KshReport, is_ksh_chart_file, looks_like_ksh};
#[cfg(feature = "std")]
pub use names::{RomanisedNames, romanised_names, romanised_names_from_bytes};
#[cfg(feature = "std")]
pub use o2jam::{OjmContainer, OjnDecoder};
#[cfg(feature = "std")]
pub use fnf_vslice::{FnfcPackage, VSliceDecoder, is_vslice_chart_name};
#[cfg(feature = "std")]
pub use qua::{QuaDecoder, QuaEncoder};
#[cfg(feature = "std")]
pub use sekai::{
    BdonDecoder, SNIFF_BYTES, SekaiReport, SusDecoder, UscDecoder, looks_like_bdon, looks_like_sus, looks_like_usc,
};
#[cfg(feature = "std")]
pub use sm_edit::EditDecoder;
#[cfg(feature = "std")]
pub use ssq::SsqDecoder;
#[cfg(feature = "std")]
pub use yrox::{YroxDecoder, YroxEncoder};

/// The index in `chart.hitsounds` of the keysound `key`, added on its first use (`sound` makes it); `seen` remembers the keys added.
#[cfg(feature = "std")]
pub(crate) fn keysound<K: std::hash::Hash + Eq>(
    chart: &mut rox::model::RoxChart,
    seen: &mut std::collections::HashMap<K, u16>,
    key: K,
    sound: impl FnOnce() -> rox::model::Hitsound,
) -> u16 {
    *seen.entry(key).or_insert_with(|| {
        chart.hitsounds.push(sound());
        u16::try_from(chart.hitsounds.len() - 1).unwrap_or(u16::MAX)
    })
}
