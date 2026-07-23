use std::sync::Mutex;

use ash::vk;

use crate::error::VulkanError;

pub struct VulkanContext {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub physical_device: vk::PhysicalDevice,
    pub device: ash::Device,
    pub graphics_queue: vk::Queue,
    pub graphics_queue_family: u32,
    pub video_queue_loader: Option<ash::khr::video_queue::Instance>,
    pub video_queue_device_loader: Option<ash::khr::video_queue::Device>,
    pub video_decode_queue_loader: Option<ash::khr::video_decode_queue::Device>,
    pub video_queue_family: Option<u32>,
    pub video_decode_queue: Option<vk::Queue>,
    pub dpb_coincide: bool,
    pub allocator: Mutex<Option<gpu_allocator::vulkan::Allocator>>,
    pub queue_mutex: Mutex<()>,
    pub video_queue_mutex: Option<Mutex<()>>,
}

impl VulkanContext {
    pub fn new() -> Result<Self, VulkanError> {
        let entry = unsafe { ash::Entry::load() }
            .map_err(|_| VulkanError::MissingExtension("vulkan-1.dll"))?;

        let instance = create_instance(&entry)?;
        let (physical_device, graphics_queue_family) = select_physical_device(&instance)?;
        let video_qf = find_video_decode_queue_family_for_device(&instance, physical_device);

        let device = create_device(&instance, physical_device, graphics_queue_family, video_qf)?;

        if let Some(vqf) = video_qf {
            tracing::info!(
                "Vulkan Video decode queue family: {} ({}graphics)",
                vqf,
                if vqf == graphics_queue_family {
                    "shared with "
                } else {
                    "separate from "
                }
            );
        } else {
            tracing::warn!(
                "No Vulkan Video decode queue family found — hardware decode unavailable"
            );
        }

        let graphics_queue = unsafe { device.get_device_queue(graphics_queue_family, 0) };
        let video_decode_queue = video_qf.map(|vqf| unsafe { device.get_device_queue(vqf, 0) });

        let allocator =
            gpu_allocator::vulkan::Allocator::new(&gpu_allocator::vulkan::AllocatorCreateDesc {
                instance: instance.clone(),
                device: device.clone(),
                physical_device,
                debug_settings: gpu_allocator::AllocatorDebugSettings::default(),
                buffer_device_address: false,
                allocation_sizes: gpu_allocator::AllocationSizes::default(),
            })
            .map_err(|e| VulkanError::Allocation(e.to_string()))?;

        let video_queue_loader = Some(ash::khr::video_queue::Instance::new(&entry, &instance));
        let video_queue_device_loader =
            Some(ash::khr::video_queue::Device::new(&instance, &device));
        let video_decode_queue_loader = Some(ash::khr::video_decode_queue::Device::new(
            &instance, &device,
        ));

        let dpb_coincide = check_dpb_coincide_capability_impl(
            &instance,
            physical_device,
            video_queue_loader.as_ref(),
        );

        let video_queue_mutex = video_qf.map(|_| Mutex::new(()));

        Ok(Self {
            entry,
            instance,
            physical_device,
            device,
            graphics_queue,
            graphics_queue_family,
            video_queue_loader,
            video_queue_device_loader,
            video_decode_queue_loader,
            video_queue_family: video_qf,
            video_decode_queue,
            dpb_coincide,
            allocator: Mutex::new(Some(allocator)),
            queue_mutex: Mutex::new(()),
            video_queue_mutex,
        })
    }

    pub fn queue_lock(&self) -> std::sync::MutexGuard<'_, ()> {
        self.queue_mutex.lock().unwrap()
    }

    pub fn video_queue_lock(&self) -> Option<std::sync::MutexGuard<'_, ()>> {
        self.video_queue_mutex.as_ref().map(|m| m.lock().unwrap())
    }

    pub fn find_video_decode_queue_family(&self) -> Option<u32> {
        find_video_decode_queue_family_for_device(&self.instance, self.physical_device)
    }

    pub fn check_dpb_coincide_capability(&self) -> bool {
        self.dpb_coincide
    }
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().ok();
            if let Ok(mut lock) = self.allocator.lock() {
                lock.take();
            }
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

const APP_NAME: &std::ffi::CStr = c"aura-wallpaperd";
const ENGINE_NAME: &std::ffi::CStr = c"aura";

fn create_instance(entry: &ash::Entry) -> Result<ash::Instance, VulkanError> {
    let app_info = vk::ApplicationInfo::default()
        .application_name(APP_NAME)
        .application_version(vk::make_api_version(0, 0, 1, 0))
        .engine_name(ENGINE_NAME)
        .engine_version(vk::make_api_version(0, 0, 1, 0))
        .api_version(vk::API_VERSION_1_3);

    let extensions = [
        ash::khr::surface::NAME.as_ptr(),
        ash::khr::win32_surface::NAME.as_ptr(),
        ash::khr::video_queue::NAME.as_ptr(),
    ];

    let validation_layer = c"VK_LAYER_KHRONOS_validation";
    let enable_validation = std::env::var("AURA_VALIDATION").as_deref() == Ok("1");

    let layers: Vec<*const i8> = if enable_validation {
        let available = unsafe { entry.enumerate_instance_layer_properties() }.unwrap_or_default();
        let has_validation = available.iter().any(|l| {
            let name = unsafe { std::ffi::CStr::from_ptr(l.layer_name.as_ptr()) };
            name == validation_layer
        });
        if has_validation {
            vec![validation_layer.as_ptr()]
        } else {
            tracing::warn!("AURA_VALIDATION=1 but VK_LAYER_KHRONOS_validation not available");
            vec![]
        }
    } else {
        vec![]
    };

    let create_info = vk::InstanceCreateInfo::default()
        .application_info(&app_info)
        .enabled_extension_names(&extensions)
        .enabled_layer_names(&layers);

    let instance = unsafe { entry.create_instance(&create_info, None)? };
    Ok(instance)
}

fn select_physical_device(
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

fn find_video_decode_queue_family_for_device(
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

fn check_dpb_coincide_capability_impl(
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

fn create_device(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    graphics_queue_family: u32,
    video_queue_family: Option<u32>,
) -> Result<ash::Device, VulkanError> {
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

    let mut extensions = Vec::with_capacity(3);
    extensions.push(ash::khr::swapchain::NAME.as_ptr());
    extensions.push(ash::khr::video_queue::NAME.as_ptr());
    extensions.push(ash::khr::video_decode_queue::NAME.as_ptr());

    if video_queue_family.is_some() {
        // Check if VK_KHR_video_decode_h264 is supported
        let available_extensions = unsafe {
            instance
                .enumerate_device_extension_properties(physical_device)
                .unwrap_or_default()
        };
        let has_h264 = available_extensions.iter().any(|e| {
            let name =
                unsafe { std::ffi::CStr::from_ptr(e.extension_name.as_ptr()) }.to_string_lossy();
            name == "VK_KHR_video_decode_h264"
        });
        if has_h264 {
            extensions.push(ash::khr::video_decode_h264::NAME.as_ptr());
        }
    }

    let create_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(&queue_infos)
        .enabled_extension_names(&extensions);

    let device = unsafe { instance.create_device(physical_device, &create_info, None)? };
    Ok(device)
}
