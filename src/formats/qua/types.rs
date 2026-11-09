#![warn(clippy::pedantic)]
use serde::{Deserialize, Serialize};

/// The game mode, `Keys<N>`: Quaver ships `Keys4` and `Keys7`, editors and forks write other counts (`Keys10`); any count from 1 is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuaMode(pub u8);

impl Default for QuaMode {
    fn default() -> Self {
        Self(4)
    }
}

impl QuaMode {
    #[must_use]
    pub const fn key_count(&self) -> u8 {
        self.0
    }
}

impl Serialize for QuaMode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("Keys{}", self.0))
    }
}

impl<'de> Deserialize<'de> for QuaMode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        name.strip_prefix("Keys")
            .and_then(|keys| keys.parse::<u8>().ok())
            .filter(|keys| *keys > 0)
            .map(Self)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown mode `{name}`, expected Keys<N>")))
    }
}

/// Beats per measure of a timing point. Quaver writes its two named values (`Quadruple`, `Triple`) by name and any other
/// count as a plain number (`Signature: 5`; maps edited by tools carry numbers such as `0` or `99999` too): a number is
/// read as that many beats, and one that is no usable count (0, or more than 255) as the default four.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimeSignature {
    #[default]
    Quadruple,
    Triple,
    Beats(u8),
}

impl TimeSignature {
    #[must_use]
    pub const fn beats(&self) -> u8 {
        match self {
            Self::Quadruple => 4,
            Self::Triple => 3,
            Self::Beats(beats) => *beats,
        }
    }
}

impl Serialize for TimeSignature {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Quadruple => serializer.serialize_str("Quadruple"),
            Self::Triple => serializer.serialize_str("Triple"),
            Self::Beats(beats) => serializer.serialize_u8(*beats),
        }
    }
}

impl<'de> Deserialize<'de> for TimeSignature {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Number(i64),
            Name(String),
        }
        Ok(match Raw::deserialize(deserializer)? {
            Raw::Name(name) if name == "Quadruple" => Self::Quadruple,
            Raw::Name(name) if name == "Triple" => Self::Triple,
            Raw::Name(name) => {
                return Err(serde::de::Error::unknown_variant(&name, &["Quadruple", "Triple"]));
            }
            Raw::Number(4) => Self::Quadruple,
            Raw::Number(3) => Self::Triple,
            Raw::Number(beats) => u8::try_from(beats).ok().filter(|beats| *beats > 0).map_or(Self::Quadruple, Self::Beats),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct QuaChart {
    #[serde(rename = "AudioFile")]
    pub audio_file: String,
    #[serde(rename = "SongPreviewTime")]
    pub preview_time: i32,
    #[serde(rename = "BackgroundFile")]
    pub background_file: Option<String>,
    #[serde(rename = "MapId")]
    pub map_id: i32,
    #[serde(rename = "MapSetId")]
    pub map_set_id: i32,
    #[serde(rename = "Mode")]
    pub mode: QuaMode,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Artist")]
    pub artist: String,
    #[serde(rename = "Source")]
    pub source: Option<String>,
    #[serde(rename = "Tags")]
    pub tags: Option<String>,
    #[serde(rename = "Creator")]
    pub creator: String,
    #[serde(rename = "DifficultyName")]
    pub difficulty_name: String,
    #[serde(rename = "TimingPoints")]
    pub timing_points: Vec<QuaTimingPoint>,
    #[serde(rename = "SliderVelocities")]
    pub slider_velocities: Vec<QuaSliderVelocity>,
    #[serde(rename = "HitObjects")]
    pub hit_objects: Vec<QuaHitObject>,
    #[serde(rename = "BPMDoesNotAffectScrollVelocity", default)]
    pub bpm_does_not_affect_sv: bool,
    #[serde(rename = "InitialScrollVelocity", default = "default_sv")]
    pub initial_scroll_velocity: f32,
}

fn default_sv() -> f32 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct QuaTimingPoint {
    #[serde(rename = "StartTime")]
    pub start_time: f64,
    #[serde(rename = "Bpm")]
    pub bpm: f32,
    #[serde(rename = "Signature", skip_serializing_if = "Option::is_none")]
    pub signature: Option<TimeSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct QuaSliderVelocity {
    #[serde(rename = "StartTime")]
    pub start_time: f64,
    #[serde(rename = "Multiplier")]
    pub multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct QuaHitObject {
    #[serde(rename = "StartTime")]
    pub start_time: f64,
    #[serde(rename = "Lane")]
    pub lane: u8,
    #[serde(rename = "EndTime", skip_serializing_if = "Option::is_none")]
    pub end_time: Option<f64>,
}
