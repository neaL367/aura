use super::ParsedH264Frame;
use super::slice::H264SliceHeader;
use super::sps::H264Sps;
use std::collections::BTreeMap;

/// Stateful Picture Order Count tracker implementing H.264 8.2.1.
pub struct PocTracker {
    prev_poc_msb: i32,
    prev_poc_lsb: i32,
}

impl Default for PocTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl PocTracker {
    pub fn new() -> Self {
        Self {
            prev_poc_msb: 0,
            prev_poc_lsb: 0,
        }
    }

    /// Compute the frame POC for the given slice, updating tracker state.
    pub fn compute(
        &mut self,
        sps: &H264Sps,
        slice: &H264SliceHeader,
        is_idr: bool,
        is_reference: bool,
    ) -> i32 {
        match sps.pic_order_cnt_type {
            0 => {
                let (poc, msb, lsb) = self.poc_type0(sps, slice, is_idr, is_reference);
                self.prev_poc_msb = msb;
                self.prev_poc_lsb = lsb;
                poc
            }
            1 => self.poc_type1(sps, slice, is_reference),
            _ => {
                // Type 2: POC derived directly from frame_num.
                let base = 2 * slice.frame_num as i32;
                if is_reference {
                    base
                } else {
                    base + sps.offset_for_non_ref_pic
                }
            }
        }
    }

    fn poc_type0(
        &self,
        sps: &H264Sps,
        slice: &H264SliceHeader,
        is_idr: bool,
        is_reference: bool,
    ) -> (i32, i32, i32) {
        let max_poc_lsb = 1i32 << sps.log2_max_pic_order_cnt_lsb();
        let (prev_msb, prev_lsb) = if is_idr {
            (0i32, 0i32)
        } else {
            (self.prev_poc_msb, self.prev_poc_lsb)
        };
        let poc_lsb = slice.pic_order_cnt_lsb.unwrap_or(0) as i32;

        let poc_msb = if poc_lsb < prev_lsb && (prev_lsb - poc_lsb) >= max_poc_lsb / 2 {
            prev_msb + max_poc_lsb
        } else if poc_lsb > prev_lsb && (poc_lsb - prev_lsb) > max_poc_lsb / 2 {
            prev_msb - max_poc_lsb
        } else {
            prev_msb
        };

        let mut poc = poc_msb + poc_lsb;
        if !is_reference {
            poc += 2 * sps.offset_for_non_ref_pic;
        }
        (poc, poc_msb, poc_lsb)
    }

    fn poc_type1(&self, sps: &H264Sps, slice: &H264SliceHeader, is_reference: bool) -> i32 {
        let mut poc = if sps.delta_pic_order_always_zero_flag {
            0
        } else {
            slice.delta_pic_order_cnt.map(|d| d[0]).unwrap_or(0)
        };
        if !is_reference {
            poc += sps.offset_for_non_ref_pic;
        }
        poc
    }
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
