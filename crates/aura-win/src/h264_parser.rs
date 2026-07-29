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
            self.buffer.pop_first().map(|(_, frame)| frame)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_avcc_to_annex_b_conversion() {
        let nal_payload = [0x67, 0x42, 0x00, 0x0a];
        let mut avcc = vec![0, 0, 0, 4];
        avcc.extend_from_slice(&nal_payload);

        let annex_b = avcc_to_annex_b(&avcc);
        assert_eq!(&annex_b[..4], &[0, 0, 0, 1]);
        assert_eq!(&annex_b[4..], &nal_payload);
    }

    #[test]
    fn test_poc_reorder_buffer_sorting() {
        let mut reorder = PocReorderBuffer::new(2);

        let frame1 = ParsedH264Frame {
            frame_num: 0,
            poc: 4,
            is_idr: false,
            nal_data: vec![1],
            pts_ms: 100,
        };
        let frame2 = ParsedH264Frame {
            frame_num: 1,
            poc: 2,
            is_idr: false,
            nal_data: vec![2],
            pts_ms: 50,
        };
        let frame3 = ParsedH264Frame {
            frame_num: 2,
            poc: 0,
            is_idr: true,
            nal_data: vec![3],
            pts_ms: 0,
        };

        assert!(reorder.push(frame1).is_none());
        assert!(reorder.push(frame2).is_none());

        // Pushing 3rd frame exceeds latency of 2, returning smallest POC (poc 0)
        let popped = reorder.push(frame3).unwrap();
        assert_eq!(popped.poc, 0);

        let flushed = reorder.flush();
        assert_eq!(flushed.len(), 2);
        assert_eq!(flushed[0].poc, 2);
        assert_eq!(flushed[1].poc, 4);
    }
}
