use ash::vk;

use crate::error::VulkanError;

// ---------------------------------------------------------------------------
// VulkanContext — shared per-process Vulkan state
// ---------------------------------------------------------------------------

/// Vulkan instance, physical device, logical device, and allocator.
///
/// One instance is created per process.  All `MonitorRenderer`s share it.
/// Must be created on the render thread and kept alive until all renderers
/// are destroyed.
pub struct VulkanContext {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub physical_device: vk::PhysicalDevice,
    pub device: ash::Device,
    pub graphics_queue: vk::Queue,
    pub graphics_queue_family: u32,
    pub allocator: gpu_allocator::vulkan::Allocator,
}

impl VulkanContext {
    /// Create a new `VulkanContext`.
    ///
    /// - Loads the Vulkan loader (`vulkan-1.dll`) via `ash::Entry::load()`.
    /// - Creates a `VkInstance` with `VK_KHR_surface` + `VK_KHR_win32_surface`.
    /// - Selects the first suitable physical device (prefers discrete GPU,
    ///   falls back to integrated).
    /// - Creates a `VkDevice` with a graphics queue that supports presentation.
    ///
    /// Validation layers are enabled when `AURA_VALIDATION=1` is set in the
    /// environment and the SDK layers are available.
    pub fn new() -> Result<Self, VulkanError> {
        // SAFETY: ash::Entry::load dynamically loads vulkan-1.dll.
        let entry = unsafe { ash::Entry::load() }
            .map_err(|_| VulkanError::MissingExtension("vulkan-1.dll"))?;

        let instance = create_instance(&entry)?;
        let (physical_device, queue_family) = select_physical_device(&instance)?;
        let device = create_device(&instance, physical_device, queue_family)?;
        let queue = unsafe { device.get_device_queue(queue_family, 0) };

        // Initialise gpu-allocator.
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

        Ok(Self {
            entry,
            instance,
            physical_device,
            device,
            graphics_queue: queue,
            graphics_queue_family: queue_family,
            allocator,
        })
    }
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        // SAFETY: All child objects must be destroyed before the device.
        // The orchestrator ensures MonitorRenderers are dropped before the context.
        unsafe {
            self.device.device_wait_idle().ok();
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
    ];

    // Enable validation layer when requested.
    let validation_layer = c"VK_LAYER_KHRONOS_validation";
    let enable_validation = std::env::var("AURA_VALIDATION").as_deref() == Ok("1");

    let layers: Vec<*const i8> = if enable_validation {
        // Check the layer is available before requesting it.
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

    // SAFETY: create_info and all pointed-to data is valid for this call.
    let instance = unsafe { entry.create_instance(&create_info, None)? };
    Ok(instance)
}

fn select_physical_device(
    instance: &ash::Instance,
) -> Result<(vk::PhysicalDevice, u32), VulkanError> {
    // SAFETY: Standard ash enumeration.
    let devices = unsafe { instance.enumerate_physical_devices()? };

    let mut best: Option<(vk::PhysicalDevice, u32, u32)> = None; // (device, queue_family, score)

    for device in devices {
        // SAFETY: Standard ash query.
        let props = unsafe { instance.get_physical_device_properties(device) };
        let queue_families =
            unsafe { instance.get_physical_device_queue_family_properties(device) };

        // Find a queue family with graphics support.
        let Some(qf_idx) = queue_families
            .iter()
            .position(|qf| qf.queue_flags.contains(vk::QueueFlags::GRAPHICS))
        else {
            continue;
        };

        // Score: discrete GPU > integrated > other.
        let score = match props.device_type {
            vk::PhysicalDeviceType::DISCRETE_GPU => 3,
            vk::PhysicalDeviceType::INTEGRATED_GPU => 2,
            vk::PhysicalDeviceType::VIRTUAL_GPU => 1,
            _ => 0,
        };

        if best.is_none() || score > best.unwrap().2 {
            best = Some((device, qf_idx as u32, score));
        }
    }

    let (device, qf, _) = best.ok_or(VulkanError::NoSuitableDevice)?;
    let props = unsafe { instance.get_physical_device_properties(device) };
    let name = unsafe { std::ffi::CStr::from_ptr(props.device_name.as_ptr()) }.to_string_lossy();
    tracing::info!("Vulkan device selected: {}", name);
    Ok((device, qf))
}

fn create_device(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    queue_family: u32,
) -> Result<ash::Device, VulkanError> {
    let queue_priority = 1.0f32;
    let queue_create_info = vk::DeviceQueueCreateInfo::default()
        .queue_family_index(queue_family)
        .queue_priorities(std::slice::from_ref(&queue_priority));

    let extensions = [ash::khr::swapchain::NAME.as_ptr()];

    let create_info = vk::DeviceCreateInfo::default()
        .queue_create_infos(std::slice::from_ref(&queue_create_info))
        .enabled_extension_names(&extensions);

    // SAFETY: Standard ash device creation.
    let device = unsafe { instance.create_device(physical_device, &create_info, None)? };
    Ok(device)
}
