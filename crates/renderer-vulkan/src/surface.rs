use ash::vk;

use crate::{context::VulkanContext, error::VulkanError};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;

/// Wraps a `VkSurfaceKHR` created from a Win32 `HWND`.
///
/// Owned by `MonitorRenderer`.  Destroyed before `VulkanContext`.
pub struct Surface {
    pub surface_loader: ash::khr::surface::Instance,
    pub surface: vk::SurfaceKHR,
}

impl Surface {
    /// Create a Win32 surface from a host window `HWND`.
    #[cfg(target_os = "windows")]
    pub fn create_win32(context: &VulkanContext, hwnd: HWND) -> Result<Self, VulkanError> {
        let win32_loader =
            ash::khr::win32_surface::Instance::new(&context.entry, &context.instance);
        let surface_loader = ash::khr::surface::Instance::new(&context.entry, &context.instance);

        let hinstance = unsafe {
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .map_err(|e| VulkanError::Win32(e.to_string()))?
        };

        let create_info = vk::Win32SurfaceCreateInfoKHR::default()
            .hinstance(hinstance.0 as isize)
            .hwnd(hwnd.0 as isize);

        let surface = unsafe {
            win32_loader
                .create_win32_surface(&create_info, None)
                .map_err(|e| VulkanError::Surface(e.to_string()))?
        };

        Ok(Self {
            surface_loader,
            surface,
        })
    }

    /// Query whether the graphics queue family supports presentation on this surface.
    pub fn get_support(
        &self,
        physical_device: vk::PhysicalDevice,
        queue_family_index: u32,
    ) -> Result<bool, VulkanError> {
        let is_supported = unsafe {
            self.surface_loader
                .get_physical_device_surface_support(
                    physical_device,
                    queue_family_index,
                    self.surface,
                )
                .map_err(|e| VulkanError::Surface(e.to_string()))?
        };
        Ok(is_supported)
    }

    /// Query physical device surface capabilities (extent limits, image count limits, transforms).
    pub fn get_capabilities(
        &self,
        physical_device: vk::PhysicalDevice,
    ) -> Result<vk::SurfaceCapabilitiesKHR, VulkanError> {
        let caps = unsafe {
            self.surface_loader
                .get_physical_device_surface_capabilities(physical_device, self.surface)
                .map_err(|e| VulkanError::Surface(e.to_string()))?
        };
        Ok(caps)
    }

    /// Query supported surface formats (e.g. `B8G8R8A8_UNORM` / `SRGB`).
    pub fn get_formats(
        &self,
        physical_device: vk::PhysicalDevice,
    ) -> Result<Vec<vk::SurfaceFormatKHR>, VulkanError> {
        let formats = unsafe {
            self.surface_loader
                .get_physical_device_surface_formats(physical_device, self.surface)
                .map_err(|e| VulkanError::Surface(e.to_string()))?
        };
        Ok(formats)
    }

    /// Query supported present modes (e.g. `FIFO`, `MAILBOX`, `IMMEDIATE`).
    pub fn get_present_modes(
        &self,
        physical_device: vk::PhysicalDevice,
    ) -> Result<Vec<vk::PresentModeKHR>, VulkanError> {
        let modes = unsafe {
            self.surface_loader
                .get_physical_device_surface_present_modes(physical_device, self.surface)
                .map_err(|e| VulkanError::Surface(e.to_string()))?
        };
        Ok(modes)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            if self.surface != vk::SurfaceKHR::null() {
                self.surface_loader.destroy_surface(self.surface, None);
                self.surface = vk::SurfaceKHR::null();
            }
        }
    }
}
