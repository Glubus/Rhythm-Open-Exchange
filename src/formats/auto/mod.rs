pub mod detect;
pub mod dispatch;
pub use dispatch::{
    FileVariant, auto_convert, auto_decode, auto_decode_all, auto_decode_bytes, auto_decode_variant, auto_encode,
};
