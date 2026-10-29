#![warn(clippy::pedantic)]
//! The channel table of a DTX chart: what each two-digit hexadecimal channel is.
//!
//! Source: the `EChannel` enumeration of `DTXmaniaNX` (`DTXMania/Code/Score,Song/EChannel.cs`, read, not copied), the
//! `vsrg-format-docs/vsrg/dtxmania/dtx` notes, and the two functions of `CDTX.cs` that tell the instrument of a chip. The guitar and
//! bass chips carry a CHORD in their channel number: the channel names the buttons held (`Guitar_RGxYx` = red, green and yellow).

/// An instrument of the file; the three of them can share one `.dtx`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Part {
    #[default]
    Drums,
    Guitar,
    Bass,
}

impl Part {
    /// The three instruments in variant order.
    pub(super) const ALL: [Self; 3] = [Self::Drums, Self::Guitar, Self::Bass];

    /// The name a variant carries: `Drums`, `Guitar`, `Bass`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Drums => "Drums",
            Self::Guitar => "Guitar",
            Self::Bass => "Bass",
        }
    }

    /// Place in [`Part::ALL`].
    pub(super) fn slot(self) -> usize {
        match self {
            Self::Drums => 0,
            Self::Guitar => 1,
            Self::Bass => 2,
        }
    }
}

/// A drum pad as the channel names it (twelve channels, `0x11..=0x1C`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Pad {
    HiHatClose,
    Snare,
    BassDrum,
    HighTom,
    LowTom,
    Cymbal,
    FloorTom,
    HiHatOpen,
    Ride,
    LeftCymbal,
    LeftPedal,
    LeftBassDrum,
}

/// Buttons of a guitar or bass chip, as bits.
pub(super) const RED: u8 = 1;
pub(super) const GREEN: u8 = 2;
pub(super) const BLUE: u8 = 4;
pub(super) const YELLOW: u8 = 8;
pub(super) const PINK: u8 = 16;
/// The open pick: no button.
pub(super) const OPEN: u8 = 32;

/// Chips the decoder reads, besides the tempo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    /// `0x03`: BPM, two hexadecimal digits (`#BASEBPM` + value).
    Bpm,
    /// `0x08`: BPM from the `#BPMzz` table.
    BpmTable,
    /// `0x01`: a background sound (BGM).
    Background,
    /// `0x61..=0x92`: a sound effect, played by itself like the BGM.
    Effect,
    Drum(Pad),
    /// A guitar or bass chip: `buttons` is a mask of [`RED`]..[`PINK`] or [`OPEN`].
    Fret {
        part: Part,
        buttons: u8,
    },
    /// `0x2C` (guitar) and `0x2D` (bass): a hold marker (the end of a hold, see `decoder.rs`).
    HoldMarker(Part),
    /// Read and dropped; counted under the name of the counter.
    Dropped(Dropped),
}

/// Why a chip is dropped: what the report counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Dropped {
    /// `0x31..=0x3C`: an invisible drum chip.
    Hidden,
    /// `0xB1..=0xBE`: the sound of a blank hit (a pad struck where no chip is).
    BlankHit,
    /// `0x28`, `0xA8` (wailing), `0x2F` (the wailing sound).
    Wailing,
    /// `0x1F`, `0x53`: a drum fill-in marker.
    FillIn,
    /// Pictures, videos, bar and beat lines.
    Visual,
    /// Known channels with no effect on the notes: bonus effects, MIDI chorus, click, mixer, reserved, flow speed.
    Inert,
    /// A channel this decoder does not know.
    Unknown,
}

/// The buttons of the three-colour chord number `v` (bit 2 red, bit 1 green, bit 0 blue: `Guitar_RGxxx` is `0b110`).
fn rgb(v: u8) -> u8 {
    (if v & 4 != 0 { RED } else { 0 })
        | (if v & 2 != 0 { GREEN } else { 0 })
        | (if v & 1 != 0 { BLUE } else { 0 })
}

/// The role of channel `channel`.
#[allow(clippy::too_many_lines)] // one table
pub(super) fn classify(channel: u8) -> Role {
    use Dropped::{BlankHit, FillIn, Hidden, Inert, Unknown, Visual, Wailing};
    let fret = |part, buttons| Role::Fret { part, buttons };
    match channel {
        0x01 => Role::Background,
        0x03 => Role::Bpm,
        0x08 => Role::BpmTable,
        0x11 => Role::Drum(Pad::HiHatClose),
        0x12 => Role::Drum(Pad::Snare),
        0x13 => Role::Drum(Pad::BassDrum),
        0x14 => Role::Drum(Pad::HighTom),
        0x15 => Role::Drum(Pad::LowTom),
        0x16 => Role::Drum(Pad::Cymbal),
        0x17 => Role::Drum(Pad::FloorTom),
        0x18 => Role::Drum(Pad::HiHatOpen),
        0x19 => Role::Drum(Pad::Ride),
        0x1A => Role::Drum(Pad::LeftCymbal),
        0x1B => Role::Drum(Pad::LeftPedal),
        0x1C => Role::Drum(Pad::LeftBassDrum),
        0x1F | 0x53 => Role::Dropped(FillIn),
        // Guitar: open, then the classic three colours, then the XG chords with yellow and/or pink.
        0x20 => fret(Part::Guitar, OPEN),
        0x21..=0x27 => fret(Part::Guitar, rgb(channel - 0x20)),
        0x93..=0x9A => fret(Part::Guitar, YELLOW | rgb(channel - 0x93)),
        0x9B..=0x9F => fret(Part::Guitar, PINK | rgb(channel - 0x9B)),
        0xA9..=0xAB => fret(Part::Guitar, PINK | rgb(5 + channel - 0xA9)),
        0xAC..=0xAF => fret(Part::Guitar, YELLOW | PINK | rgb(channel - 0xAC)),
        0xD0..=0xD3 => fret(Part::Guitar, YELLOW | PINK | rgb(4 + channel - 0xD0)),
        // Bass: the same chords on their own channels.
        0xA0 => fret(Part::Bass, OPEN),
        0xA1..=0xA7 => fret(Part::Bass, rgb(channel - 0xA0)),
        0xC5 | 0xC6 => fret(Part::Bass, YELLOW | rgb(channel - 0xC5)),
        0xC8..=0xCD => fret(Part::Bass, YELLOW | rgb(2 + channel - 0xC8)),
        0xCE | 0xCF => fret(Part::Bass, PINK | rgb(channel - 0xCE)),
        0xDA..=0xDF => fret(Part::Bass, PINK | rgb(2 + channel - 0xDA)),
        0xE1..=0xE8 => fret(Part::Bass, YELLOW | PINK | rgb(channel - 0xE1)),
        0x2C => Role::HoldMarker(Part::Guitar),
        0x2D => Role::HoldMarker(Part::Bass),
        0x28 | 0x2F | 0xA8 => Role::Dropped(Wailing),
        0x31..=0x3C => Role::Dropped(Hidden),
        0xB1..=0xBE => Role::Dropped(BlankHit),
        0x61..=0x69 | 0x70..=0x79 | 0x80..=0x89 | 0x90..=0x92 => Role::Effect,
        // Pictures (BGA layers and their swap variants), videos, bar and beat lines and their settings.
        0x04
        | 0x07
        | 0x50
        | 0x51
        | 0x54..=0x5A
        | 0x60
        | 0xC1
        | 0xC2
        | 0xC4
        | 0xC7
        | 0xD5..=0xD9
        | 0xE0 => Role::Dropped(Visual),
        // Reserved, unused, flow speed, bonus effects, MIDI chorus, mixer channels, click, first sound chip, mixer add/remove.
        0x05
        | 0x06
        | 0x09
        | 0x0A
        | 0x29
        | 0x30
        | 0x4C..=0x4F
        | 0x52
        | 0x5B..=0x5F
        | 0xEA..=0xEF => Role::Dropped(Inert),
        _ => Role::Dropped(Unknown),
    }
}
