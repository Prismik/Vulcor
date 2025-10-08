use anyhow::{Result};
use std::{ffi::CString};
use ash::{vk, Device};
use cgmath::{vec2, vec3};

use crate::{
    math::vector::Vec2, 
    math::vector::Vec3,
    pipeline::{shader::Shader, traits::VulkanPipeline}, 
    swapchain::SwapchainConfig
};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    pos: Vec2,
    color: Vec3,
}

impl Vertex {
    const fn new(p: Vec2, color: Vec3) -> Self {
        Self { pos: p, color }
    }

    fn binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(size_of::<Vertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)
    }

    fn attribute_descriptions() -> [vk::VertexInputAttributeDescription; 2] {
        let p_desc = vk::VertexInputAttributeDescription::default()
            .binding(0)
            .location(0)
            .format(vk::Format::R32G32_SFLOAT)
            .offset(0);

        let color_desc = vk::VertexInputAttributeDescription::default()
            .binding(0)
            .location(1)
            .format(vk::Format::R32G32B32_SFLOAT)
            .offset(size_of::<Vec2>() as u32);

        [p_desc, color_desc]
    }
}

pub static VERTICES: [Vertex; 4] = [
    Vertex::new(vec2(-0.5, -0.5), vec3(1.0, 0.0, 0.0)),
    Vertex::new(vec2(0.5, -0.5), vec3(0.0, 1.0, 0.0)),
    Vertex::new(vec2(0.5, 0.5), vec3(0.0, 0.0, 1.0)),
    Vertex::new(vec2(-0.5, 0.5), vec3(1.0, 1.0, 1.0)),
];

pub static INDICES: &[u16] = &[0, 1, 2, 2, 3, 0];

pub struct RenderPipeline {
    vk_instance: vk::Pipeline,
    vk_layout: vk::PipelineLayout
}

impl RenderPipeline {
    fn create_layout(logical_device: &Device) -> Result<vk::PipelineLayout> {
        let layout_info = vk::PipelineLayoutCreateInfo::default();
        let layout = unsafe { logical_device.create_pipeline_layout(&layout_info, None)? };
        Ok(layout)
    }
}

impl VulkanPipeline for RenderPipeline {
    fn new(logical_device: &Device, config: &SwapchainConfig, render_pass: &vk::RenderPass) -> Result<Self> {
        let vert = Shader::new("shaders/shader.vert.spv", logical_device)?;
        let frag = Shader::new("shaders/shader.frag.spv", logical_device)?;
        let main = CString::new("main")?;
        let vert_stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vert.instance)
            .name(main.as_c_str());
        let frag_stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(frag.instance)
            .name(main.as_c_str());

        let binding_descriptions = &[Vertex::binding_description()];
        let attribute_descriptions = Vertex::attribute_descriptions();
        let vert_input_state = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(binding_descriptions)
            .vertex_attribute_descriptions(&attribute_descriptions);
        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        let viewport = vk::Viewport::default()
            .x(0.0)
            .y(0.0)
            .width(config.extent.width as f32)
            .height(config.extent.height as f32)
            .min_depth(0.0)
            .max_depth(1.0);
        let scissor = vk::Rect2D::default()
            .offset(vk::Offset2D { x: 0, y: 0 })
            .extent(config.extent);
        let viewports = &[viewport];
        let scissors = &[scissor];
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewports(viewports)
            .scissors(scissors);
        let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::CLOCKWISE)
            .depth_bias_enable(false);

        let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        // Blending can be changed here
        let color_blend_attachment_state = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ZERO)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD);
        let attachments = &[color_blend_attachment_state];
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op_enable(false)
            .logic_op(vk::LogicOp::COPY)
            .attachments(attachments)
            .blend_constants([0.0, 0.0, 0.0, 0.0]);
        
        let layout = Self::create_layout(logical_device)?;
        let stages = &[vert_stage, frag_stage];
        let graphics_pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(stages)
            .vertex_input_state(&vert_input_state)
            .input_assembly_state(&input_assembly_state)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization_state)
            .multisample_state(&multisample_state)
            .color_blend_state(&color_blend_state)
            .layout(layout)
            .render_pass(*render_pass)
            .subpass(0)
            .base_pipeline_handle(vk::Pipeline::null())
            .base_pipeline_index(-1);
        let pipeline = unsafe { 
            logical_device.create_graphics_pipelines(
                vk::PipelineCache::null(), 
                &[graphics_pipeline_info], 
                None
            ).as_ref().unwrap()[0]
        };
        unsafe { logical_device.destroy_shader_module(vert.instance, None) };
        unsafe { logical_device.destroy_shader_module(frag.instance, None) };

        Ok(Self{vk_instance: pipeline, vk_layout: layout})
    }

    fn instance(&self) -> vk::Pipeline {
        self.vk_instance
    }
    
    fn layout(&self) -> vk::PipelineLayout {
        self.vk_layout
    }

    fn cleanup(&self, logical_device: &Device) {
        unsafe { logical_device.destroy_pipeline(self.vk_instance, None); }
        unsafe { logical_device.destroy_pipeline_layout(self.vk_layout, None); }
    }

}