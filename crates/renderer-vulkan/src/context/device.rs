use ash::vk;

use crate::error::VulkanError;

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

    let create_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(&queue_infos)
        .enabled_extension_names(&extensions);

    let device = unsafe { instance.create_device(physical_device, &create_info, None)? };
    Ok((device, video_extensions_enabled))
}
