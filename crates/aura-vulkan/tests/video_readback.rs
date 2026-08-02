use aura_vulkan::video_readback::nv12_to_rgba;

#[test]
fn nv12_conversion_smoke() {
    let w = 4u32;
    let h = 4u32;
    let mut nv12 = vec![0u8; (w * h * 3 / 2) as usize];
    // Grey frame: Y=128, U=V=128 -> RGB should be ~ (119, 119, 119).
    for p in nv12.iter_mut() {
        *p = 128;
    }
    let rgba = nv12_to_rgba(&nv12, w, h).unwrap();
    assert_eq!(rgba.len(), (w * h * 4) as usize);
    for px in rgba.chunks_exact(4).take(4) {
        assert_eq!(px[0], 130);
        assert_eq!(px[1], 130);
        assert_eq!(px[2], 130);
        assert_eq!(px[3], 255);
    }
}

#[test]
fn nv12_conversion_red() {
    // Pure red in BT.709 limited range: Y=63, U=102, V=240.
    let w = 2u32;
    let h = 2u32;
    let mut nv12 = vec![0u8; (w * h * 3 / 2) as usize];
    nv12[0] = 63;
    nv12[1] = 63;
    nv12[2] = 63;
    nv12[3] = 63;
    nv12[4] = 102; // U (first of pair)
    nv12[5] = 240; // V
    let rgba = nv12_to_rgba(&nv12, w, h).unwrap();
    assert_eq!(rgba[0], 255);
    assert!(rgba[1] <= 1, "green = {}", rgba[1]);
    assert!(rgba[2] <= 1, "blue = {}", rgba[2]);
}

#[test]
fn nv12_conversion_too_small() {
    assert!(nv12_to_rgba(&[0u8; 3], 4, 4).is_err());
}
