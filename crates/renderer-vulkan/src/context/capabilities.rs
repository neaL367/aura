use ash::vk;

pub fn check_dpb_coincide_capability_impl(
    _instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    video_loader: Option<&ash::khr::video_queue::Instance>,
) -> bool {
    let Some(loader) = video_loader else {
        return true;
    };

    let mut h264_profile = vk::VideoDecodeH264ProfileInfoKHR::default()
        .std_profile_idc(ash::vk::native::StdVideoH264ProfileIdc_STD_VIDEO_H264_PROFILE_IDC_MAIN);

    let profile_info = vk::VideoProfileInfoKHR::default()
        .video_codec_operation(vk::VideoCodecOperationFlagsKHR::DECODE_H264)
        .push_next(&mut h264_profile);

    let mut capabilities_chain = vk::VideoCapabilitiesKHR::default();

    let result = unsafe {
        (loader.fp().get_physical_device_video_capabilities_khr)(
            physical_device,
            &profile_info as *const _,
            &mut capabilities_chain as *mut _,
        )
    };
    let result: Result<(), vk::Result> = if result == vk::Result::SUCCESS {
        Ok(())
    } else {
        Err(result)
    };

    match result {
        Ok(()) => {
            let coincide = !capabilities_chain
                .flags
                .contains(vk::VideoCapabilityFlagsKHR::SEPARATE_REFERENCE_IMAGES);
            tracing::info!(
                "Vulkan Video DPB-coincide: {} (flags: {:?})",
                coincide,
                capabilities_chain.flags
            );
            coincide
        }
        Err(e) => {
            tracing::warn!(
                "Failed to query video capabilities, assuming DPB coincide: {}",
                e
            );
            true
        }
    }
}
