use core::panic;
use std::sync::Arc;

use anyhow::{Result};
use ash::vk::{self, DescriptorSet};

use crate::{core::{graphics::Graphics, logical_device::GraphicsInterface}, pipeline::{render_pipeline::{INDICES, VERTICES}, traits::VulkanPipeline}, resources::buffer::Buffer, swapchain::SwapchainData};

pub struct CmdPool {
    pub instance: vk::CommandPool
}

impl CmdPool {
    pub fn new(device: &GraphicsInterface, queue_family: u32) -> Result<Self> {
        let create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::empty())
            .queue_family_index(queue_family);

        let command_pool = unsafe { device.instance.create_command_pool(&create_info, None)? };
        Ok(Self { instance: command_pool })
    }

    pub unsafe fn create_buffers(&self, device: &GraphicsInterface, render_pass: &vk::RenderPass, pipeline: &dyn VulkanPipeline, framebuffers: &Vec<vk::Framebuffer>, vertex_buffer: &Buffer, index_buffer: &Buffer, swapchain: &SwapchainData, descriptor_sets: &Vec<DescriptorSet>) -> Result<Vec<vk::CommandBuffer>> {
        let count = framebuffers.len() as u32;
        let allocate_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.instance)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(count);

        let buffers = device.instance.allocate_command_buffers(&allocate_info)?;
        for (i, command_buffer) in buffers.iter().enumerate() {
            let inheritance = vk::CommandBufferInheritanceInfo::default();
            let info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::empty())
                .inheritance_info(&inheritance);
            
            let render_area = vk::Rect2D::default()
                .offset(vk::Offset2D::default())
                .extent(swapchain.config.extent);
            let clear_color_value = vk::ClearValue {
                color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 1.0] }
            };
            let clear_values = &[clear_color_value];
            let begin_info = vk::RenderPassBeginInfo::default()
                .render_pass(*render_pass)
                .framebuffer(framebuffers[i])
                .render_area(render_area)
                .clear_values(clear_values);

            // Setup commands
            device.instance.begin_command_buffer(*command_buffer, &info)?;
            device.instance.cmd_begin_render_pass(*command_buffer, &begin_info, vk::SubpassContents::INLINE);
            device.instance.cmd_bind_pipeline(*command_buffer, vk::PipelineBindPoint::GRAPHICS, pipeline.instance());
            device.instance.cmd_bind_vertex_buffers(*command_buffer, 0, &[vertex_buffer.instance], &[0]);
            device.instance.cmd_bind_index_buffer(*command_buffer, index_buffer.instance, 0, vk::IndexType::UINT16);
            device.instance.cmd_bind_descriptor_sets(*command_buffer, vk::PipelineBindPoint::GRAPHICS, pipeline.layout(), 0, &[descriptor_sets[i]], &[]);
            device.instance.cmd_draw_indexed(*command_buffer, INDICES.len() as u32, 1, 0, 0, 0);
            device.instance.cmd_end_render_pass(*command_buffer);
            device.instance.end_command_buffer(*command_buffer)?;
        };

        Ok(buffers)
    }
}