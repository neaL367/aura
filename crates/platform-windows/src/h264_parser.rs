//! H.264 bitstream parser adapter and Picture Order Count (POC) reorder buffer.
//!
//! Extracts SPS, PPS, frame numbers, and POC metrics using `h264-reader`,
//! and provides a parser-only diff test against reference frame metadata.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedH264Frame {
    pub frame_num: u32,
    pub poc: i32,
    pub is_idr: bool,
    pub nal_data: Vec<u8>,
    pub pts_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub struct H264SpsInfo {
    pub profile_idc: u8,
    pub level_idc: u8,
    pub width: u32,
    pub height: u32,
    pub max_num_ref_frames: u32,
    pub pic_order_cnt_type: u8,
    pub log2_max_frame_num: u8,
    pub log2_max_pic_order_cnt_lsb: u8,
}

/// Convert AVCC 4-byte length-prefixed NAL units into Annex-B `0x00000001` start code bitstream.
pub fn avcc_to_annex_b(avcc_data: &[u8]) -> Vec<u8> {
    let mut annex_b = Vec::with_capacity(avcc_data.len() + 16);
    let mut offset = 0;

    while offset + 4 <= avcc_data.len() {
        let length = u32::from_be_bytes([
            avcc_data[offset],
            avcc_data[offset + 1],
            avcc_data[offset + 2],
            avcc_data[offset + 3],
        ]) as usize;

        offset += 4;
        if offset + length > avcc_data.len() {
            // Malformed length; append remaining raw bytes and stop
            annex_b.extend_from_slice(&avcc_data[offset - 4..]);
            break;
        }

        annex_b.extend_from_slice(&[0, 0, 0, 1]);
        annex_b.extend_from_slice(&avcc_data[offset..offset + length]);
        offset += length;
    }

    if annex_b.is_empty() && !avcc_data.is_empty() {
        // Fallback if not length-prefixed
        annex_b.extend_from_slice(&[0, 0, 0, 1]);
        annex_b.extend_from_slice(avcc_data);
    }

    annex_b
}

/// Picture Order Count (POC) Reorder Buffer for ordering decoded frames into display-order.
pub struct PocReorderBuffer {
    buffer: BTreeMap<i32, ParsedH264Frame>,
    max_reorder_latency: usize,
}

impl PocReorderBuffer {
    pub fn new(max_reorder_latency: usize) -> Self {
        Self {
            buffer: BTreeMap::new(),
            max_reorder_latency,
        }
    }

    /// Push a decoded frame in decode order. Returns the next frame ready for display, if available.
    pub fn push(&mut self, frame: ParsedH264Frame) -> Option<ParsedH264Frame> {
        let poc = frame.poc;
        self.buffer.insert(poc, frame);

        if self.buffer.len() > self.max_reorder_latency {
            let smallest_poc = *self.buffer.keys().next().unwrap();
            self.buffer.remove(&smallest_poc)
        } else {
            None
        }
    }

    /// Flush remaining frames in display-order upon stream EOF or loop reset.
    pub fn flush(&mut self) -> Vec<ParsedH264Frame> {
        let mut frames = Vec::with_capacity(self.buffer.len());
        while let Some((_, frame)) = self.buffer.pop_first() {
            frames.push(frame);
        }
        frames
    }
}
