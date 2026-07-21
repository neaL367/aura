use ash::vk;

use crate::{context::VulkanContext, error::VulkanError};

/// Vulkan Graphics Pipeline for rendering quad wallpaper textures.
pub struct GraphicsPipeline {
    pub descriptor_set_layout: vk::DescriptorSetLayout,
    pub pipeline_layout: vk::PipelineLayout,
    pub render_pass: vk::RenderPass,
    pub pipeline: vk::Pipeline,
}

impl GraphicsPipeline {
    /// Create a graphics pipeline for rendering a textured 2D quad onto `color_format`.
    pub fn create(context: &VulkanContext, color_format: vk::Format) -> Result<Self, VulkanError> {
        // 1. Create descriptor set layout (binding 0: combined image sampler)
        let layout_binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT);

        let layout_info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(std::slice::from_ref(&layout_binding));

        let descriptor_set_layout = unsafe {
            context
                .device
                .create_descriptor_set_layout(&layout_info, None)
                .map_err(|e| VulkanError::Pipeline(e.to_string()))?
        };

        // 2. Create pipeline layout
        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(std::slice::from_ref(&descriptor_set_layout));

        let pipeline_layout = unsafe {
            context
                .device
                .create_pipeline_layout(&pipeline_layout_info, None)
                .map_err(|e| VulkanError::Pipeline(e.to_string()))?
        };

        // 3. Create render pass
        let color_attachment = vk::AttachmentDescription::default()
            .format(color_format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

        let color_attachment_ref = vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(std::slice::from_ref(&color_attachment_ref));

        let subpass_dependency = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            );

        let render_pass_info = vk::RenderPassCreateInfo::default()
            .attachments(std::slice::from_ref(&color_attachment))
            .subpasses(std::slice::from_ref(&subpass))
            .dependencies(std::slice::from_ref(&subpass_dependency));

        let render_pass = unsafe {
            context
                .device
                .create_render_pass(&render_pass_info, None)
                .map_err(|e| VulkanError::Pipeline(e.to_string()))?
        };

        // Null pipeline placeholder until shader compilation step
        let pipeline = vk::Pipeline::null();

        Ok(Self {
            descriptor_set_layout,
            pipeline_layout,
            render_pass,
            pipeline,
        })
    }

    /// Destroy pipeline handles and layout objects.
    ///
    /// # Safety
    /// Must be called when GPU execution using this pipeline has completed.
    pub unsafe fn destroy(&mut self, device: &ash::Device) {
        unsafe {
            if self.pipeline != vk::Pipeline::null() {
                device.destroy_pipeline(self.pipeline, None);
            }
            device.destroy_render_pass(self.render_pass, None);
            device.destroy_pipeline_layout(self.pipeline_layout, None);
            device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
        }
    }
}
