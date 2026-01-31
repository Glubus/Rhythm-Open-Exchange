use serde::{Deserialize, Serialize};

/// Named classifications for 2x2 cell patterns.
///
/// # Why 2x2 Grid?
/// We use a 2x2 grid (2 columns, 2 time slices) as the fundamental atomic unit for pattern recognition because:
/// 1. It is the smallest unit that can capture temporal relationships (jacks, streams).
/// 2. It maps cleanly to QuadTrees for recursive analysis.
/// 3. It allows efficient bitwise classification (4 bits = 16 states).
///
/// Grid layout: [TL][TR] (top = earlier time)
///              [BL][BR] (bottom = later time)
/// Binary encoding: TL TR BL BR (4 bits)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum PatternClassification {
    // Empty pattern
    Empty = 0b0000,

    // Single note patterns
    SingleTL = 0b1000,
    SingleTR = 0b0100,
    SingleBL = 0b0010,
    SingleBR = 0b0001,

    // Horizontal pairs - same time, different columns (jumps)
    JumpTop = 0b1100,
    JumpBottom = 0b0011,

    // Vertical pairs - same column, different times (jacks)
    JackLeft = 0b1010,
    JackRight = 0b0101,

    // Diagonal patterns (trills/streams)
    TrillDown = 0b1001,
    TrillUp = 0b0110,

    // Triple patterns (3 notes)
    TripleNoTL = 0b0111,
    TripleNoTR = 0b1011,
    TripleNoBL = 0b1101,
    TripleNoBR = 0b1110,

    // Full chord
    Chord = 0b1111,
}

impl PatternClassification {
    /// Create classification from 4 boolean cell values.
    pub fn from_grid(tl: bool, tr: bool, bl: bool, br: bool) -> Self {
        let binary = (if tl { 8 } else { 0 })
            | (if tr { 4 } else { 0 })
            | (if bl { 2 } else { 0 })
            | (if br { 1 } else { 0 });
        match binary {
            0b0000 => Self::Empty,
            0b1000 => Self::SingleTL,
            0b0100 => Self::SingleTR,
            0b0010 => Self::SingleBL,
            0b0001 => Self::SingleBR,
            0b1100 => Self::JumpTop,
            0b0011 => Self::JumpBottom,
            0b1010 => Self::JackLeft,
            0b0101 => Self::JackRight,
            0b1001 => Self::TrillDown,
            0b0110 => Self::TrillUp,
            0b0111 => Self::TripleNoTL,
            0b1011 => Self::TripleNoTR,
            0b1101 => Self::TripleNoBL,
            0b1110 => Self::TripleNoBR,
            0b1111 => Self::Chord,
            _ => unreachable!(),
        }
    }

    /// Count number of notes in this pattern.
    pub fn note_count(&self) -> u32 {
        (*self as u8).count_ones()
    }

    /// Check if pattern has no notes.
    pub fn is_empty(&self) -> bool {
        *self == Self::Empty
    }

    /// Check if pattern contains a horizontal pair (jump).
    pub fn has_jump(&self) -> bool {
        matches!(
            self,
            Self::JumpTop
                | Self::JumpBottom
                | Self::TripleNoTL
                | Self::TripleNoTR
                | Self::TripleNoBL
                | Self::TripleNoBR
                | Self::Chord
        )
    }

    /// Check if pattern contains a vertical pair (jack).
    pub fn has_jack(&self) -> bool {
        matches!(
            self,
            Self::JackLeft
                | Self::JackRight
                | Self::TripleNoTL
                | Self::TripleNoTR
                | Self::TripleNoBL
                | Self::TripleNoBR
                | Self::Chord
        )
    }

    /// Get the general category of this classification.
    pub fn get_category(&self) -> PatternCategory {
        match self {
            Self::Empty => PatternCategory::Empty,
            Self::SingleTL | Self::SingleTR | Self::SingleBL | Self::SingleBR => {
                PatternCategory::Single
            }
            Self::JumpTop | Self::JumpBottom => PatternCategory::Jump,
            Self::JackLeft | Self::JackRight => PatternCategory::Jack,
            Self::TrillDown | Self::TrillUp => PatternCategory::Trill,
            Self::TripleNoTL | Self::TripleNoTR | Self::TripleNoBL | Self::TripleNoBR => {
                PatternCategory::Triple
            }
            Self::Chord => PatternCategory::Chord,
        }
    }
}

/// Coarse categories for pattern classification.
///
/// # Why Categories?
/// While specific classifications (like `JackLeft`) are useful for low-level analysis,
/// higher-level logic (like detecting "Stream Sections") requires grouping these atomics
/// into broader categories. This simplifies the merge logic in the QuadTree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PatternCategory {
    Empty,
    Single,
    Jump,
    Jack,
    Trill,
    Triple,
    Chord,
}

/// High-level pattern types recognized from multiple classifications.
///
/// # Why this hierarchy?
/// These types represent the VSRG community's standard taxonomy for patterns.
/// They range from atomic patterns (Stream) to complex hybrids (Jumpstream, Chordjack).
/// The definitions align with the "Quattern" library to ensure familiarity for osu!mania players.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PatternType {
    // Empty
    EmptyRegion,
    VerySparse,

    // Single notes
    SingleNotes,
    Scattered,
    SparseSingles,

    // Streams
    Stream,
    ReverseStream,
    StreamSection,
    SparseStream,
    StreamWithSingles,
    StreamDense,

    // Jumps
    JumpSection,
    SparseJumps,
    JumpWithSingles,
    LightJumps,
    DenseJumps,
    AlternatingJumps,

    // Jacks
    JackSection,
    ExtendedJackLeft,
    ExtendedJackRight,
    SplitJack,
    SparseJacks,
    JackWithSingles,
    LightJacks,

    // Chords/Triples
    ChordSection,
    SparseChords,
    ChordWithSingles,
    LightChords,
    DenseChord,
    TripleSection,
    TripleWithSingles,

    // Hybrids (Technical)
    TechnicalHybrid,
    TechnicalWithSingles,
    SparseTechnical,
    Jumpstream,
    JumpstreamDense,
    JumpstreamWithSingles,
    Handstream,
    HandstreamDense,
    Chordjack,
    ChordjackDense,

    // Generic
    Mixed,
    ComplexMixed,
    ComplexDense,
    Dense,
    Moderate,
    Light,
}

impl PatternType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EmptyRegion => "EmptyRegion",
            Self::VerySparse => "VerySparse",
            Self::SingleNotes => "SingleNotes",
            Self::Scattered => "Scattered",
            Self::SparseSingles => "SparseSingles",
            Self::Stream => "Stream",
            Self::ReverseStream => "ReverseStream",
            Self::StreamSection => "StreamSection",
            Self::SparseStream => "SparseStream",
            Self::StreamWithSingles => "StreamWithSingles",
            Self::StreamDense => "StreamDense",
            Self::JumpSection => "JumpSection",
            Self::SparseJumps => "SparseJumps",
            Self::JumpWithSingles => "JumpWithSingles",
            Self::LightJumps => "LightJumps",
            Self::DenseJumps => "DenseJumps",
            Self::AlternatingJumps => "AlternatingJumps",
            Self::JackSection => "JackSection",
            Self::ExtendedJackLeft => "ExtendedJackLeft",
            Self::ExtendedJackRight => "ExtendedJackRight",
            Self::SplitJack => "SplitJack",
            Self::SparseJacks => "SparseJacks",
            Self::JackWithSingles => "JackWithSingles",
            Self::LightJacks => "LightJacks",
            Self::ChordSection => "ChordSection",
            Self::SparseChords => "SparseChords",
            Self::ChordWithSingles => "ChordWithSingles",
            Self::LightChords => "LightChords",
            Self::DenseChord => "DenseChord",
            Self::TripleSection => "TripleSection",
            Self::TripleWithSingles => "TripleWithSingles",
            Self::TechnicalHybrid => "TechnicalHybrid",
            Self::TechnicalWithSingles => "TechnicalWithSingles",
            Self::SparseTechnical => "SparseTechnical",
            Self::Jumpstream => "Jumpstream",
            Self::JumpstreamDense => "JumpstreamDense",
            Self::JumpstreamWithSingles => "JumpstreamWithSingles",
            Self::Handstream => "Handstream",
            Self::HandstreamDense => "HandstreamDense",
            Self::Chordjack => "Chordjack",
            Self::ChordjackDense => "ChordjackDense",
            Self::Mixed => "Mixed",
            Self::ComplexMixed => "ComplexMixed",
            Self::ComplexDense => "ComplexDense",
            Self::Dense => "Dense",
            Self::Moderate => "Moderate",
            Self::Light => "Light",
        }
    }
}
