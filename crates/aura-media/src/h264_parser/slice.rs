use super::H264ParseError;
use super::bitreader::BitReader;
use super::sps::H264Sps;

#[derive(Debug, Clone)]
pub struct H264SliceHeader {
    pub first_mb_in_slice: u32,
    pub slice_type: u32,
    pub pps_id: u8,
    pub frame_num: u32,
    pub field_pic_flag: bool,
    pub bottom_field_flag: bool,
    pub idr_pic_id: u32,
    pub pic_order_cnt_lsb: Option<u32>,
    pub delta_pic_order_cnt_bottom: Option<i32>,
    pub delta_pic_order_cnt: Option<[i32; 2]>,
}

/// Parse a slice header NAL payload. Only fields needed for POC and
/// reference tracking are extracted; the remainder is skipped.
pub fn parse_slice_header(
    payload: &[u8],
    sps: &H264Sps,
    is_idr: bool,
) -> Result<H264SliceHeader, H264ParseError> {
    let mut r = BitReader::new(payload);
    let first_mb_in_slice = r.ue()?;
    let slice_type = r.ue()?;
    let pps_id = r.ue()? as u8;
    let frame_num = r.u(sps.log2_max_frame_num() as usize)?;

    let field_pic_flag = if !sps.frame_mbs_only_flag {
        r.u(1)? == 1
    } else {
        false
    };
    let bottom_field_flag = if field_pic_flag { r.u(1)? == 1 } else { false };

    let idr_pic_id = if is_idr { r.ue()? } else { 0 };

    let (pic_order_cnt_lsb, delta_pic_order_cnt_bottom, delta_pic_order_cnt) =
        if sps.pic_order_cnt_type == 0 {
            (
                Some(r.u(sps.log2_max_pic_order_cnt_lsb() as usize)?),
                None,
                None,
            )
        } else if sps.pic_order_cnt_type == 1 && !sps.delta_pic_order_always_zero_flag {
            let d0 = r.se()?;
            let d1 = if sps.delta_pic_order_always_zero_flag {
                0
            } else {
                r.se()?
            };
            (None, None, Some([d0, d1]))
        } else {
            (None, None, None)
        };

    Ok(H264SliceHeader {
        first_mb_in_slice,
        slice_type,
        pps_id,
        frame_num,
        field_pic_flag,
        bottom_field_flag,
        idr_pic_id,
        pic_order_cnt_lsb,
        delta_pic_order_cnt_bottom,
        delta_pic_order_cnt,
    })
}
