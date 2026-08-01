/// Remove emulation prevention bytes (`00 00 03 -> 00 00`) from a NAL payload.
pub fn remove_emulation_prevention(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut zero_count = 0;
    let mut i = 0;
    while i < data.len() {
        if zero_count >= 2 && data[i] == 0x03 && i + 1 < data.len() && data[i + 1] <= 0x03 {
            zero_count = 0;
            i += 1;
            continue;
        }
        if data[i] == 0 {
            zero_count += 1;
        } else {
            zero_count = 0;
        }
        out.push(data[i]);
        i += 1;
    }
    out
}

/// Split an Annex-B bitstream into NAL payloads (after the start code).
pub fn split_annex_b_nal_units(data: &[u8]) -> Vec<Vec<u8>> {
    let mut nals = Vec::new();
    let mut start = None;
    let mut i = 0;
    while i + 3 <= data.len() {
        let is_start = data[i] == 0
            && data[i + 1] == 0
            && (data[i + 2] == 1 || (data[i + 2] == 0 && i + 3 < data.len() && data[i + 3] == 1));
        if is_start {
            let code_len = if i + 3 < data.len() && data[i + 3] == 1 {
                4
            } else {
                3
            };
            if let Some(s) = start {
                nals.push(data[s..i].to_vec());
            }
            start = Some(i + code_len);
            i += code_len;
            continue;
        }
        i += 1;
    }
    if let Some(s) = start {
        nals.push(data[s..].to_vec());
    }
    nals
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
