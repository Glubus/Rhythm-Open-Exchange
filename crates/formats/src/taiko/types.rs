#![warn(clippy::pedantic)]
#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use bitflags::bitflags;
use crate::osu::types::{OsuDifficulty, OsuGeneral, OsuMetadata, OsuTimingPoint};

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TaikoHitsound: u32 {
        const NORMAL  = 1 << 0;
        const WHISTLE = 1 << 1;
        const FINISH  = 1 << 2;
        const CLAP    = 1 << 3;
    }
}

impl TaikoHitsound {
    #[must_use] pub fn is_kat(self) -> bool { self.contains(Self::WHISTLE) || self.contains(Self::CLAP) }
    #[must_use] pub fn is_big(self) -> bool { self.contains(Self::FINISH) }
}

#[derive(Debug, Clone)]
pub struct TaikoHitObject {
    pub time_ms: f64,
    pub hitsound: TaikoHitsound,
    pub object_type: u32,
}

impl TaikoHitObject {
    #[must_use] pub fn is_spinner(&self) -> bool { (self.object_type & 8) != 0 }
}

#[derive(Debug, Default)]
pub struct TaikoBeatmap {
    pub format_version: u8,
    pub general: OsuGeneral,
    pub metadata: OsuMetadata,
    pub difficulty: OsuDifficulty,
    pub background: Option<String>,
    pub timing_points: Vec<OsuTimingPoint>,
    pub hit_objects: Vec<TaikoHitObject>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnLayout { #[default] Dkkd, Dkdk, Kddk }

impl ColumnLayout {
    #[must_use] pub const fn don_columns(self) -> [u8; 2] {
        match self { Self::Dkkd => [0, 3], Self::Dkdk => [0, 2], Self::Kddk => [1, 2] }
    }
    #[must_use] pub const fn kat_columns(self) -> [u8; 2] {
        match self { Self::Dkkd => [1, 2], Self::Dkdk => [1, 3], Self::Kddk => [0, 3] }
    }
}

#[derive(Debug)]
pub struct AlternationState {
    pub layout: ColumnLayout,
    pub don_index: usize,
    pub kat_index: usize,
}

impl Default for AlternationState {
    fn default() -> Self { Self::new(ColumnLayout::default()) }
}

impl AlternationState {
    #[must_use] pub const fn new(layout: ColumnLayout) -> Self {
        Self { layout, don_index: 0, kat_index: 0 }
    }

    pub fn next_don_columns(&mut self, is_big: bool) -> Vec<u8> {
        let cols = self.layout.don_columns();
        if is_big { vec![cols[0], cols[1]] }
        else { let c = cols[self.don_index]; self.don_index = (self.don_index + 1) % 2; vec![c] }
    }

    pub fn next_kat_columns(&mut self, is_big: bool) -> Vec<u8> {
        let cols = self.layout.kat_columns();
        if is_big { vec![cols[0], cols[1]] }
        else { let c = cols[self.kat_index]; self.kat_index = (self.kat_index + 1) % 2; vec![c] }
    }
}
