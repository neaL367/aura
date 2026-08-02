pub mod capabilities;
pub mod device;
pub mod instance;

use std::sync::Mutex;

use ash::vk;

use capabilities::check_dpb_coincide_capability_impl;
use device::{create_device, find_video_decode_queue_family_for_device, select_physical_device};
use instance::create_instance;

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
        tracing::info!("Vulkan entry loaded");

        let instance = create_instance(&entry)?;
        tracing::info!("Vulkan instance created");
        let (physical_device, graphics_queue_family) = select_physical_device(&instance)?;
        tracing::info!(
            "Vulkan physical device selected, graphics queue family {}",
            graphics_queue_family
        );
        let video_qf = find_video_decode_queue_family_for_device(&instance, physical_device);
        tracing::info!("Vulkan video decode queue family: {:?}", video_qf);

        let (device, video_extensions_enabled) =
            create_device(&instance, physical_device, graphics_queue_family, video_qf)?;
        tracing::info!(
            "Vulkan logical device created, video extensions enabled={}",
            video_extensions_enabled
        );

        if let Some(vqf) = video_qf {
            tracing::info!(
                "Vulkan Video decode queue family: {} ({}graphics), extensions_enabled={}",
                vqf,
                if vqf == graphics_queue_family {
                    "shared with "
                } else {
                    "separate from "
                },
                video_extensions_enabled
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

        // Only create video loaders when the extensions were actually enabled in
        // the device. Loading function pointers for extensions that were not
        // enabled results in null/undefined function pointers that crash on call.
        let (video_queue_loader, video_queue_device_loader, video_decode_queue_loader) =
            if video_extensions_enabled {
                (
                    Some(ash::khr::video_queue::Instance::new(&entry, &instance)),
                    Some(ash::khr::video_queue::Device::new(&instance, &device)),
                    Some(ash::khr::video_decode_queue::Device::new(
                        &instance, &device,
                    )),
                )
            } else {
                tracing::info!("Vulkan Video extensions not enabled — video loaders disabled");
                (None, None, None)
            };

        // Guard capability probe behind extension availability; calling the
        // function pointer when extensions are absent is undefined behavior.
        let dpb_coincide = if video_extensions_enabled {
            check_dpb_coincide_capability_impl(
                &instance,
                physical_device,
                video_queue_loader.as_ref(),
            )
        } else {
            true // default safe value when video is unavailable
        };

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
        self.queue_mutex.lock().unwrap_or_else(|e| {
            tracing::warn!("Queue mutex poisoned, recovering");
            e.into_inner()
        })
    }

    pub fn video_queue_lock(&self) -> Option<std::sync::MutexGuard<'_, ()>> {
        self.video_queue_mutex.as_ref().map(|m| {
            m.lock().unwrap_or_else(|e| {
                tracing::warn!("Video queue mutex poisoned, recovering");
                e.into_inner()
            })
        })
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
