//! Vulkan Video Decode Pipeline (Stage 4 & 5).
//!
//! Submits `vkCmdBeginVideoCodingKHR` -> `vkCmdControlVideoCodingKHR` (reset
//! on first use / loop) -> `vkCmdDecodeVideoKHR` -> `vkCmdEndVideoCodingKHR`
//! on the video decode queue and synchronizes execution with the graphics
//! presentation queue via Timeline Semaphores.
//!
//! Bitstream data is staged through a ring of host-visible buffers (M2b
//! pipelining): decodes run asynchronously, gated only by the staging ring,
//! so the video queue can be one decode ahead while the graphics queue
//! samples the previous frame.
//!
//! DPB slots live in `VIDEO_DECODE_DPB_KHR` layout for decoding (setup
//! reference slot aliases the decode output picture, so VUID 07253 requires
//! DPB rather than DST layout). When a frame is presented by the graphics
//! queue it is transitioned to `SHADER_READ_ONLY_OPTIMAL`; the worker marks
//! the slot sampled via `mark_slot_sampled` (on renderer ack) and the next
//! decode into that slot transitions it back.
//!
//! Cross-queue safety: the graphics queue signals `gfx_timeline` after
//! sampling a slot (value reported in the slot-reuse ack); the decode
//! worker waits on it before overwriting the slot, so a pipelined decode
//! can never clobber a frame that is still being sampled.

mod bookkeeping;
mod create;
mod destroy;
mod record;
mod staging;
mod submit;
mod types;

pub use types::{
    DecodeFrameInput, DecodedVideoFrame, DpbSlotState, GpuAck, GpuVideoMessage, ReferencePicture,
    VideoDecodePipeline, VideoGpuFrame,
};
