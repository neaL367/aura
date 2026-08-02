use ash::vk;

use crate::error::VulkanError;

/// Mirror of `VkPhysicalDeviceVideoDecodeH264FeaturesKHR`.
///
/// ash 0.38 never generated this struct (or its `StructureType` variant)
/// even though the rest of `VK_EXT_video_decode_h264` is present. The
/// layout matches the C definition exactly; `std_syntax_only` tells the
/// driver we only submit well-formed H.264 standard-syntax bitstreams.
#[repr(C)]
#[derive(Copy, Clone)]
struct H264DecodeFeaturesKhr {
    s_type: vk::StructureType,
    p_next: *mut std::ffi::c_void,
    std_syntax_only: u32,
}

impl H264DecodeFeaturesKhr {
    fn new(std_syntax_only: bool) -> Self {
        Self {
            s_type: unsafe { std::mem::transmute::<i32, vk::StructureType>(1000185000i32) },
            p_next: std::ptr::null_mut(),
            std_syntax_only: u32::from(std_syntax_only),
        }
    }
}

pub fn select_physical_device(
    instance: &ash::Instance,
) -> Result<(vk::PhysicalDevice, u32), VulkanError> {
    let devices = unsafe { instance.enumerate_physical_devices()? };

    let mut best: Option<(vk::PhysicalDevice, u32, u32)> = None;

    for device in devices {
        let props = unsafe { instance.get_physical_device_properties(device) };
        let queue_families =
            unsafe { instance.get_physical_device_queue_family_properties(device) };

        let Some(qf_idx) = queue_families
            .iter()
            .position(|qf| qf.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        else {
            continue;
        };

        let score = match props.device_type {
            vk::PhysicalDeviceType::DISCRETE_GPU => 3,
            vk::PhysicalDeviceType::INTEGRATED_GPU => 2,
            vk::PhysicalDeviceType::VIRTUAL_GPU => 1,
            _ => 0,
        };

        if best.is_none_or(|(_, _, best_score)| score > best_score) {
            best = Some((device, qf_idx as u32, score));
        }
    }

    let (device, qf, _) = best.ok_or(VulkanError::NoSuitableDevice)?;
    let props = unsafe { instance.get_physical_device_properties(device) };
    let name = unsafe { std::ffi::CStr::from_ptr(props.device_name.as_ptr()) }.to_string_lossy();
    tracing::info!("Vulkan device selected: {}", name);
    Ok((device, qf))
}

pub fn find_video_decode_queue_family_for_device(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
) -> Option<u32> {
    let queue_families =
        unsafe { instance.get_physical_device_queue_family_properties(physical_device) };
    queue_families
        .iter()
        .position(|qf| {
            qf.queue_flags
                .contains(vk::QueueFlags::from_raw(0x00000020))
        })
        .map(|idx| idx as u32)
}

/// Create the logical device and return it alongside a flag indicating whether
/// Vulkan Video extensions (`VK_KHR_video_queue`, `VK_KHR_video_decode_queue`)
/// were actually enabled. Callers must check this flag before loading video
/// function pointers or probing video capabilities.
pub fn create_device(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    graphics_queue_family: u32,
    video_queue_family: Option<u32>,
) -> Result<(ash::Device, bool), VulkanError> {
    let queue_priority = 1.0f32;

    let mut queue_infos = Vec::with_capacity(2);
    queue_infos.push(
        vk::DeviceQueueCreateInfo::default()
            .queue_family_index(graphics_queue_family)
            .queue_priorities(std::slice::from_ref(&queue_priority)),
    );

    let shares_graphics = video_queue_family == Some(graphics_queue_family);
    if let Some(vqf) = video_queue_family
        && !shares_graphics
        && vqf != graphics_queue_family
    {
        queue_infos.push(
            vk::DeviceQueueCreateInfo::default()
                .queue_family_index(vqf)
                .queue_priorities(std::slice::from_ref(&queue_priority)),
        );
    }

    let mut extensions = Vec::with_capacity(4);
    extensions.push(ash::khr::swapchain::NAME.as_ptr());
    let mut video_extensions_enabled = false;

    if video_queue_family.is_some() {
        let available_extensions = unsafe {
            instance
                .enumerate_device_extension_properties(physical_device)
                .unwrap_or_default()
        };

        let has_ext = |ext_name: &std::ffi::CStr| {
            available_extensions.iter().any(|e| {
                let name = unsafe { std::ffi::CStr::from_ptr(e.extension_name.as_ptr()) };
                name == ext_name
            })
        };

        if has_ext(ash::khr::video_queue::NAME) && has_ext(ash::khr::video_decode_queue::NAME) {
            extensions.push(ash::khr::video_queue::NAME.as_ptr());
            extensions.push(ash::khr::video_decode_queue::NAME.as_ptr());
            if has_ext(ash::khr::video_decode_h264::NAME) {
                extensions.push(ash::khr::video_decode_h264::NAME.as_ptr());
            }
            video_extensions_enabled = true;
        } else {
            tracing::warn!(
                "VK_KHR_video_queue or VK_KHR_video_decode_queue not available on this \
                 device — Vulkan Video hardware decode disabled"
            );
        }
    }

    // Probe device API version; only enable core features the device supports.
    let props = unsafe { instance.get_physical_device_properties(physical_device) };
    let api_version = props.api_version;
    let has_vulkan_1_1 = api_version >= vk::API_VERSION_1_1;
    let has_vulkan_1_2 = api_version >= vk::API_VERSION_1_2;
    let has_vulkan_1_3 = api_version >= vk::API_VERSION_1_3;

    if !has_vulkan_1_1 {
        tracing::warn!(
            "Device only supports Vulkan {} — video decode and YCbCr sampling unavailable",
            api_version
        );
    }

    let mut vulkan13_features = if has_vulkan_1_3 {
        vk::PhysicalDeviceVulkan13Features::default().synchronization2(true)
    } else {
        vk::PhysicalDeviceVulkan13Features::default()
    };
    let mut vulkan12_features = if has_vulkan_1_2 {
        vk::PhysicalDeviceVulkan12Features::default().timeline_semaphore(true)
    } else {
        vk::PhysicalDeviceVulkan12Features::default()
    };
    let mut vulkan11_features = if has_vulkan_1_1 {
        vk::PhysicalDeviceVulkan11Features::default().sampler_ycbcr_conversion(true)
    } else {
        vk::PhysicalDeviceVulkan11Features::default()
    };

    // Query device features first, then only enable the H.264 decode feature
    // chain if the device reports support. Passing the H264 decode feature
    // struct unconditionally has been observed to hang some drivers inside
    // vkCreateDevice, so we gate it on the explicit feature query.
    let mut h264_features_opt: Option<H264DecodeFeaturesKhr> = None;
    if video_extensions_enabled && has_vulkan_1_1 {
        let mut probe_h264 = H264DecodeFeaturesKhr::new(true);
        let mut probe_features2 = vk::PhysicalDeviceFeatures2 {
            p_next: &mut probe_h264 as *mut H264DecodeFeaturesKhr as *mut _,
            ..Default::default()
        };
        unsafe {
            instance.get_physical_device_features2(physical_device, &mut probe_features2);
        }
        if probe_h264.std_syntax_only != 0 {
            h264_features_opt = Some(probe_h264);
        } else {
            tracing::warn!(
                "VK_KHR_video_decode_h264 supported but std_syntax_only is not — disabling H.264 decode"
            );
        }
    }

    let mut features2 = if let Some(ref mut h264_features) = h264_features_opt {
        vulkan11_features.p_next = h264_features as *mut H264DecodeFeaturesKhr as *mut _;
        if has_vulkan_1_3 {
            vk::PhysicalDeviceFeatures2::default()
                .push_next(&mut vulkan13_features)
                .push_next(&mut vulkan12_features)
                .push_next(&mut vulkan11_features)
        } else if has_vulkan_1_2 {
            vk::PhysicalDeviceFeatures2::default()
                .push_next(&mut vulkan12_features)
                .push_next(&mut vulkan11_features)
        } else {
            vk::PhysicalDeviceFeatures2::default().push_next(&mut vulkan11_features)
        }
    } else if has_vulkan_1_3 {
        vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut vulkan13_features)
            .push_next(&mut vulkan12_features)
            .push_next(&mut vulkan11_features)
    } else if has_vulkan_1_2 {
        vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut vulkan12_features)
            .push_next(&mut vulkan11_features)
    } else if has_vulkan_1_1 {
        vk::PhysicalDeviceFeatures2::default().push_next(&mut vulkan11_features)
    } else {
        vk::PhysicalDeviceFeatures2::default()
    };

    let create_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(&queue_infos)
        .enabled_extension_names(&extensions)
        .push_next(&mut features2);

    let device = unsafe { instance.create_device(physical_device, &create_info, None)? };
    Ok((device, video_extensions_enabled))
}
