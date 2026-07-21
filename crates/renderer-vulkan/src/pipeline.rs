use ash::vk;

use crate::{context::VulkanContext, error::VulkanError};

/// Vulkan Graphics Pipeline for rendering full-screen textured wallpaper quads.
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

        // 4. Create shader modules from embedded SPIR-V bytecode
        let vert_code = crate::shader::vertex_shader_spv();
        let frag_code = crate::shader::fragment_shader_spv();

        let vert_words = ash::util::read_spv(&mut std::io::Cursor::new(vert_code))
            .map_err(|e| VulkanError::ShaderCompilation(e.to_string()))?;
        let frag_words = ash::util::read_spv(&mut std::io::Cursor::new(frag_code))
            .map_err(|e| VulkanError::ShaderCompilation(e.to_string()))?;

        let vert_module_info = vk::ShaderModuleCreateInfo::default().code(&vert_words);
        let frag_module_info = vk::ShaderModuleCreateInfo::default().code(&frag_words);

        let vert_module = unsafe {
            context
                .device
                .create_shader_module(&vert_module_info, None)
                .map_err(|e| VulkanError::ShaderCompilation(e.to_string()))?
        };

        let frag_module = unsafe {
            context
                .device
                .create_shader_module(&frag_module_info, None)
                .map_err(|e| VulkanError::ShaderCompilation(e.to_string()))?
        };

        let main_name = c"main";
        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vert_module)
                .name(main_name),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(frag_module)
                .name(main_name),
        ];

        // 5. Create Graphics Pipeline state
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);

        let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE);

        let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(
                vk::ColorComponentFlags::R
                    | vk::ColorComponentFlags::G
                    | vk::ColorComponentFlags::B
                    | vk::ColorComponentFlags::A,
            )
            .blend_enable(false);

        let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op_enable(false)
            .attachments(std::slice::from_ref(&color_blend_attachment));

        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state =
            vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);

        let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default();
        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input_info)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisampling)
            .color_blend_state(&color_blending)
            .dynamic_state(&dynamic_state)
            .layout(pipeline_layout)
            .render_pass(render_pass)
            .subpass(0);

        let pipeline = unsafe {
            context
                .device
                .create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    std::slice::from_ref(&pipeline_info),
                    None,
                )
                .map_err(|(_, e)| VulkanError::Pipeline(e.to_string()))?[0]
        };

        // Clean up temporary shader module handles
        unsafe {
            context.device.destroy_shader_module(vert_module, None);
            context.device.destroy_shader_module(frag_module, None);
        }

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
