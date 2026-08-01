use ash::vk;

pub struct DecodedVideoFrame {
    pub image: vk::Image,
    pub image_view: vk::ImageView,
    /// Timeline value signaled when the decode submission finished.
    pub timeline_value: u64,
    /// DPB slot index holding the frame.
    pub slot_index: u32,
}

/// GPU-side video frame message sent from the decode worker to the render
/// thread for direct DPB sampling (no host readback).
#[derive(Clone, Copy)]
pub struct VideoGpuFrame {
    /// DPB image (for layout transitions on the graphics queue).
    pub image: vk::Image,
    /// DPB image view (multi-planar NV12, YCbCr conversion chained).
    pub image_view: vk::ImageView,
    /// Timeline semaphore the decode worker signals on decode completion.
    pub timeline_semaphore: vk::Semaphore,
    /// Value of `timeline_semaphore` when this frame finished decoding.
    pub timeline_value: u64,
    /// Graphics->video timeline signaled after the renderer samples this
    /// frame (guards slot overwrite by later decodes).
    pub gfx_timeline: vk::Semaphore,
    /// DPB slot holding the frame (must not be reused until acked).
    pub slot_index: u32,
    /// Coded (aligned) frame dimensions in pixels.
    pub width: u32,
    pub height: u32,
    pub pts_ms: u64,
    pub duration_ms: u64,
}

/// Messages sent from the decode worker to the render thread over the GPU
/// frame channel.
#[derive(Clone, Copy)]
pub enum GpuVideoMessage {
    /// A decoded NV12 picture ready for direct DPB sampling.
    Frame(VideoGpuFrame),
    /// The decode worker is recreating the video session (SPS/PPS
    /// renegotiation) and will destroy its DPB images: the renderer must
    /// drop its active image view before that happens, then acknowledge
    /// with `GpuAck::SessionReset` on the ack channel.
    SessionReset,
}

/// A reference picture used by a decode operation.
pub struct ReferencePicture {
    pub slot_index: u32,
    pub frame_num: u16,
    pub pic_order_cnt: [i32; 2],
}

/// Inputs for decoding a single H.264 picture (access unit).
pub struct DecodeFrameInput<'a> {
    /// Annex-B access unit containing only the picture's slices.
    pub bitstream: &'a [u8],
    /// Byte offsets of each slice NAL start within `bitstream`.
    pub slice_offsets: &'a [u32],
    pub seq_parameter_set_id: u8,
    pub pic_parameter_set_id: u8,
    pub frame_num: u16,
    pub idr_pic_id: u16,
    pub pic_order_cnt: [i32; 2],
    pub is_idr: bool,
    pub is_reference: bool,
    /// DPB slot index the decoded picture is written into.
    pub setup_slot_index: u32,
    /// Active reference pictures for this frame (empty for IDR / I slices).
    pub references: &'a [ReferencePicture],
}

/// Picture currently occupying a DPB slot, for reference slot mapping.
#[derive(Clone, Copy)]
pub struct DpbSlotState {
    pub frame_num: u16,
    pub pic_order_cnt: [i32; 2],
}

/// Number of in-flight decodes (staging buffer + command buffer pairs).
/// M2b pipelines decodes: the video queue can execute the next picture while
/// the previous one is still being sampled by the graphics queue.
pub(super) const DECODE_RING_SIZE: usize = 2;

/// One in-flight decode slot: a host-visible staging buffer paired with a
/// command buffer. A slot is reused only after the decode that last used it
/// (`last_value`) completes, so consecutive decodes never overwrite a
/// bitstream the video decoder is still reading.
pub(super) struct DecodeStagingSlot {
    /// Bitstream staging buffer (`VIDEO_DECODE_SRC_KHR`).
    pub(super) buffer: vk::Buffer,
    pub(super) allocation: Option<gpu_allocator::vulkan::Allocation>,
    pub(super) capacity: u64,
    pub(super) command_buffer: vk::CommandBuffer,
    /// Timeline value of the decode that last used this slot (0 = unused).
    pub(super) last_value: u64,
}

/// Ack messages from the render thread to the decode worker over the
/// slot-ack channel.
#[derive(Clone, Copy, Debug)]
pub enum GpuAck {
    /// The displayed frame in `slot` is no longer sampled; `gfx_value` is
    /// the graphics-timeline value signaled after the renderer sampled the
    /// slot (the worker waits for it before overwriting the slot).
    SlotReused { slot: u32, gfx_value: u64 },
    /// The renderer dropped its active video frame for a session reset.
    SessionReset,
}

/// Video decode pipeline coordinator for H.264 execution and queue synchronization.
pub struct VideoDecodePipeline {
    pub decode_command_pool: vk::CommandPool,
    pub timeline_semaphore: vk::Semaphore,
    pub timeline_value: u64,
    /// Graphics->video timeline semaphore: the renderer signals it after
    /// sampling a DPB slot, so a later decode never overwrites a slot that
    /// is still being sampled.
    pub gfx_timeline: vk::Semaphore,
    /// Ring of in-flight decode slots (bitstream staging + command buffer).
    pub(super) staging_slots: Vec<DecodeStagingSlot>,
    /// Next staging slot to use (round-robin over `staging_slots`).
    pub(super) ring_head: usize,
    /// Per-slot picture bookkeeping for reference slot mapping.
    pub(super) slot_state: Vec<Option<DpbSlotState>>,
    /// True until the first `vkCmdControlVideoCodingKHR` reset is recorded.
    pub(super) session_reset_required: bool,
    /// Tracked layout of each DPB slot (UNDEFINED before first use;
    /// `SHADER_READ_ONLY_OPTIMAL` after graphics-side sampling).
    pub(super) slot_layouts: Vec<vk::ImageLayout>,
    /// Graphics-timeline value last signaled after sampling each DPB slot
    /// (0 = never sampled).
    pub(super) gfx_sampled: Vec<u64>,
}
