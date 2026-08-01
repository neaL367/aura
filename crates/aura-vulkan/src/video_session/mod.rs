//! Vulkan Video H.264 decode session.
//!
//! Creates the real `VkVideoSessionKHR` (with H.264 profile chain), the
//! session parameters populated from parsed SPS/PPS via `StdVideoH264`
//! structs, a shared `VkSamplerYcbcrConversion` for NV12 sampling, and the
//! DPB image array sized `max_num_ref_frames + 1`. DPB images are
//! multi-planar (`G8_B8R8_2PLANE_420_UNORM`) and CONCURRENT-shared between
//! the video decode and graphics queue families when they differ.

use std::os::raw::c_char;

use ash::vk;
use aura_media::h264_parser::{H264Pps, H264Sps};

use crate::{
    context::VulkanContext,
    error::VulkanError,
    h264_std_video::{StdH264Pps, StdH264Sps, h264_profile_idc},
};

const NV12: vk::Format = vk::Format::G8_B8R8_2PLANE_420_UNORM;

/// `StdVideoH264_Version` — the header version advertised in the session
/// create info (`VK_MAKE_VIDEO_STD_VERSION(1, 0, 0)` = `1 << 22`).
fn std_h264_header_version() -> vk::ExtensionProperties {
    const NAME: &[u8] = b"VK_STD_VULKAN_VIDEO_CODEC_H264_STD_VERSION_1_0_0\0";
    let mut extension_name = [0 as c_char; vk::MAX_EXTENSION_NAME_SIZE];
    for (dst, src) in extension_name.iter_mut().zip(NAME) {
        *dst = *src as c_char;
    }
    vk::ExtensionProperties {
        extension_name,
        spec_version: 1 << 22,
    }
}

/// Decoded Picture Buffer (DPB) image slot.
pub struct DpbSlot {
    pub image: vk::Image,
    /// Combined COLOR-aspect view over both planes, chained with the
    /// session's `VkSamplerYcbcrConversion` (hardware NV12 sampling).
    pub view: vk::ImageView,
    pub allocation: Option<gpu_allocator::vulkan::Allocation>,
}

/// Manages a `VkVideoSessionKHR`, its parameters, and the DPB array.
pub struct VulkanVideoSession {
    pub session: vk::VideoSessionKHR,
    pub session_parameters: vk::VideoSessionParametersKHR,
    pub ycbcr_conversion: vk::SamplerYcbcrConversion,
    pub dpb_slots: Vec<DpbSlot>,
    pub width: u32,
    pub height: u32,
    pub max_ref_frames: u32,
    /// Counter for `vkUpdateVideoSessionParametersKHR` calls: each update
    /// must specify the current counter plus one (VUID 07215).
    parameters_update_sequence: u32,
}

mod destroy;
mod dpb;

use dpb::create_dpb_slots;

impl VulkanVideoSession {
    /// Create a video decode session for the given SPS/PPS pair.
    ///
    /// `width`/`height` are the coded (aligned) frame dimensions.
    pub fn create(
        context: &VulkanContext,
        sps: &H264Sps,
        pps: &H264Pps,
        width: u32,
        height: u32,
    ) -> Result<Self, VulkanError> {
        let Some(loader) = context.video_queue_device_loader.as_ref() else {
            return Err(VulkanError::MissingExtension("VK_KHR_video_queue"));
        };
        let Some(queue_family) = context.video_queue_family else {
            return Err(VulkanError::MissingExtension("video decode queue family"));
        };

        let max_ref_frames = sps.max_num_ref_frames.max(1) as u32;
        let dpb_capacity = max_ref_frames + 1;

        // -- Session ------------------------------------------------------
        let mut h264_profile = vk::VideoDecodeH264ProfileInfoKHR::default()
            .std_profile_idc(h264_profile_idc(sps.profile_idc))
            .picture_layout(vk::VideoDecodeH264PictureLayoutFlagsKHR::PROGRESSIVE);

        let profile_info = vk::VideoProfileInfoKHR::default()
            .video_codec_operation(vk::VideoCodecOperationFlagsKHR::DECODE_H264)
            .push_next(&mut h264_profile);

        let std_version = std_h264_header_version();

        let session_create = vk::VideoSessionCreateInfoKHR::default()
            .queue_family_index(queue_family)
            .flags(vk::VideoSessionCreateFlagsKHR::empty())
            .video_profile(&profile_info)
            .picture_format(NV12)
            .max_coded_extent(vk::Extent2D { width, height })
            .reference_picture_format(NV12)
            .max_dpb_slots(dpb_capacity)
            .max_active_reference_pictures(max_ref_frames)
            .std_header_version(&std_version);

        let mut session = vk::VideoSessionKHR::null();
        let result = unsafe {
            (loader.fp().create_video_session_khr)(
                context.device.handle(),
                &session_create as *const _,
                std::ptr::null(),
                &mut session,
            )
        };
        if result != vk::Result::SUCCESS {
            return Err(VulkanError::Video(format!(
                "vkCreateVideoSessionKHR failed: {result:?}"
            )));
        }

        // -- Session parameters --------------------------------------------
        let std_sps = StdH264Sps::from_sps(sps)?;
        let std_sps_raw = std_sps.as_std();
        let std_pps = StdH264Pps::from_pps(pps);
        let std_pps_raw = std_pps.as_std();

        let add_info = vk::VideoDecodeH264SessionParametersAddInfoKHR::default()
            .std_sp_ss(std::slice::from_ref(&std_sps_raw))
            .std_pp_ss(std::slice::from_ref(&std_pps_raw));

        let mut params_create = vk::VideoDecodeH264SessionParametersCreateInfoKHR::default()
            .max_std_sps_count(16)
            .max_std_pps_count(16)
            .parameters_add_info(&add_info);

        let base_params_create = vk::VideoSessionParametersCreateInfoKHR::default()
            .video_session(session)
            .push_next(&mut params_create);

        let mut session_parameters = vk::VideoSessionParametersKHR::null();
        let result = unsafe {
            (loader.fp().create_video_session_parameters_khr)(
                context.device.handle(),
                &base_params_create as *const _,
                std::ptr::null(),
                &mut session_parameters,
            )
        };
        if result != vk::Result::SUCCESS {
            return Err(VulkanError::Video(format!(
                "vkCreateVideoSessionParametersKHR failed: {result:?}"
            )));
        }

        // -- YCbCr conversion (NV12 -> RGB, BT.709, narrow range) ----------
        let ycbcr_conversion_create = vk::SamplerYcbcrConversionCreateInfo::default()
            .format(NV12)
            .ycbcr_model(vk::SamplerYcbcrModelConversion::YCBCR_709)
            .ycbcr_range(vk::SamplerYcbcrRange::ITU_NARROW)
            .components(vk::ComponentMapping {
                r: vk::ComponentSwizzle::IDENTITY,
                g: vk::ComponentSwizzle::IDENTITY,
                b: vk::ComponentSwizzle::IDENTITY,
                a: vk::ComponentSwizzle::IDENTITY,
            })
            .x_chroma_offset(vk::ChromaLocation::MIDPOINT)
            .y_chroma_offset(vk::ChromaLocation::MIDPOINT)
            .chroma_filter(vk::Filter::NEAREST)
            .force_explicit_reconstruction(false);

        let ycbcr_conversion = unsafe {
            context
                .device
                .create_sampler_ycbcr_conversion(&ycbcr_conversion_create, None)
                .map_err(|e| VulkanError::Video(format!("vkCreateSamplerYcbcrConversion: {e:?}")))?
        };

        // -- DPB images ------------------------------------------------
        let dpb_slots = create_dpb_slots(
            context,
            width,
            height,
            queue_family,
            dpb_capacity,
            ycbcr_conversion,
        )?;
        tracing::info!(
            "VulkanVideoSession created: {}x{} (profile {:?}), DPB slots: {}",
            width,
            height,
            std_sps_raw.profile_idc,
            dpb_capacity
        );

        Ok(Self {
            session,
            session_parameters,
            ycbcr_conversion,
            dpb_slots,
            width,
            height,
            max_ref_frames,
            parameters_update_sequence: 0,
        })
    }

    /// Add new SPS/PPS entries to the session parameters object.
    ///
    /// Updates are add-only: each entry must use a `seq_parameter_set_id` /
    /// `pic_parameter_set_id` not yet present in the session (VUID 07216),
    /// and the update sequence count must equal the current counter plus
    /// one (VUID 07215). Incompatible changes (re-used ids, new extent,
    /// different DPB capacity or profile) require a full session recreation
    /// instead.
    pub fn update_parameters(
        &mut self,
        context: &VulkanContext,
        sps: Option<&H264Sps>,
        pps: Option<&H264Pps>,
    ) -> Result<(), VulkanError> {
        let Some(loader) = context.video_queue_device_loader.as_ref() else {
            return Err(VulkanError::MissingExtension("VK_KHR_video_queue"));
        };

        // The Std* values must outlive `update_info`: `add_info` stores raw
        // pointers into them until the vkUpdate call.
        let sps_std: Option<StdH264Sps> = sps.map(StdH264Sps::from_sps).transpose()?;
        let pps_std: Option<StdH264Pps> = pps.map(StdH264Pps::from_pps);
        let sps_raw = sps_std.as_ref().map(StdH264Sps::as_std);
        let pps_raw = pps_std.as_ref().map(StdH264Pps::as_std);

        let mut add_info = vk::VideoDecodeH264SessionParametersAddInfoKHR::default();
        if let Some(raw) = &sps_raw {
            add_info = add_info.std_sp_ss(std::slice::from_ref(raw));
        }
        if let Some(raw) = &pps_raw {
            add_info = add_info.std_pp_ss(std::slice::from_ref(raw));
        }

        self.parameters_update_sequence += 1;
        let update_info = vk::VideoSessionParametersUpdateInfoKHR::default()
            .update_sequence_count(self.parameters_update_sequence)
            .push_next(&mut add_info);

        let result = unsafe {
            (loader.fp().update_video_session_parameters_khr)(
                context.device.handle(),
                self.session_parameters,
                &update_info,
            )
        };
        if result != vk::Result::SUCCESS {
            return Err(VulkanError::Video(format!(
                "vkUpdateVideoSessionParametersKHR failed: {result:?}"
            )));
        }
        Ok(())
    }
}
