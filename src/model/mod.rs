//! Data model for ROX format.

mod chart;
mod hitsound;
mod metadata;
mod note;
mod timing;

pub use chart::{RoxChart, ROX_MAGIC, ROX_VERSION};
pub use hitsound::Hitsound;
pub use metadata::Metadata;
pub use note::{Note, NoteType};
pub use timing::TimingPoint;
