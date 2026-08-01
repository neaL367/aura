//! `aura-media` — Media decoding abstractions.
//!
//! Defines the `MediaDecoder` trait and concrete decoders for static images
//! and animated GIFs, plus pure-Rust H.264 bitstream parsing (SPS/PPS/slice
//! headers, POC tracking, Annex-B/AVCC conversion) shared by the Vulkan
//! Video pipeline. Video decoding is implemented in `aura-platform-windows`
//! (Media Foundation) and plugged in via the `MediaDecoder` trait.

pub mod decoder;
pub mod error;
pub mod frame_queue;
pub mod gif_decoder;
pub mod h264_parser;
pub mod image_decoder;
pub mod video_decoder;

pub use decoder::{DecodedFrame, MediaDecoder};
pub use error::MediaError;
pub use frame_queue::{FrameReceiver, FrameSender, frame_channel};
pub use gif_decoder::GifDecoder;
pub use image_decoder::ImageDecoder;
pub use video_decoder::is_video_by_extension;
