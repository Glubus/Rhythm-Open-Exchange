//! FFI API for C# bindings.
//! Standalone crate for FFI.
//! Decoupled into multiple submodules.

#[macro_use]
pub mod macros;
pub mod analysis;
pub mod codec;
pub mod memory;
pub mod metadata;
pub mod notes;
pub mod types;

pub use analysis::*;
pub use codec::*;
pub use memory::*;
pub use metadata::*;
pub use notes::*;
pub use types::*;
