//! H.264 bitstream parsing, POC tracking, and display-order reordering.
//!
//! Self-contained RBSP parser (no external decoder dependency) that extracts
//! the syntax elements required for Vulkan Video H.264 decode: SPS/PPS for
//! `VkStdVideoH264SequenceParameterSet`/`PictureParameterSet` construction,
//! and per-slice frame_num/POC for display-order reordering.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedH264Frame {
    pub frame_num: u32,
    pub poc: i32,
    pub is_idr: bool,
    pub nal_data: Vec<u8>,
    pub pts_ms: u64,
}

mod annexb;
mod bitreader;
mod poc;
mod pps;
mod slice;
mod sps;

pub use annexb::{avcc_to_annex_b, remove_emulation_prevention, split_annex_b_nal_units};
pub use bitreader::BitReader;
pub use poc::{PocReorderBuffer, PocTracker};
pub use pps::{H264Pps, parse_pps};
pub use slice::{H264SliceHeader, parse_slice_header};
pub use sps::{H264Sps, parse_sps};

#[derive(Debug, thiserror::Error)]
pub enum H264ParseError {
    #[error("H.264 bitstream truncated while parsing")]
    Truncated,
    #[error("Invalid H.264 syntax")]
    InvalidSyntax,
    #[error("Unsupported H.264 feature: {0}")]
    Unsupported(String),
}
