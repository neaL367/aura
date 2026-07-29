use ash::vk;

use crate::error::VulkanError;

const APP_NAME: &std::ffi::CStr = c"aura-wallpaperd";
const ENGINE_NAME: &std::ffi::CStr = c"aura";

pub fn create_instance(entry: &ash::Entry) -> Result<ash::Instance, VulkanError> {
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
