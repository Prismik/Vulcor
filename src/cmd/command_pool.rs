use std::sync::Arc;

use anyhow::{Result};
use ash::vk;

use crate::{core::{graphics::Graphics, logical_device::GraphicsInterface}, pipeline::render_pipeline::{INDICES, VERTICES}, resources::buffer::Buffer, swapchain::SwapchainData};

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

    pub unsafe fn create_buffers(&self, count: u32, device: &GraphicsInterface, render_pass: &vk::RenderPass, pipeline: &vk::Pipeline, framebuffers: &Vec<vk::Framebuffer>, vertex_buffer: &Buffer, index_buffer: &Buffer, swapchain: &SwapchainData) -> Result<Vec<vk::CommandBuffer>> {
        let allocate_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(self.instance)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(count);

        let buffers = device.instance.allocate_command_buffers(&allocate_info)?;
        buffers.iter()
            .zip(framebuffers.iter())
            .for_each(|(command_buffer, framebuffer)| {
                let inheritance = vk::CommandBufferInheritanceInfo::default();
                let info = vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::empty())
                    .inheritance_info(&inheritance);
                let _ = device.instance.begin_command_buffer(*command_buffer, &info);
                let render_area = vk::Rect2D::default()
                    .offset(vk::Offset2D::default())
                    .extent(swapchain.config.extent);
                let clear_color_value = vk::ClearValue {
                    color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 1.0] }
                };
                let clear_values = &[clear_color_value];
                let begin_info = vk::RenderPassBeginInfo::default()
                    .render_pass(*render_pass)
                    .framebuffer(*framebuffer)
                    .render_area(render_area)
                    .clear_values(clear_values);

                // Setup commands
                device.instance.cmd_begin_render_pass(*command_buffer, &begin_info, vk::SubpassContents::INLINE);
                device.instance.cmd_bind_pipeline(*command_buffer, vk::PipelineBindPoint::GRAPHICS, *pipeline);
                device.instance.cmd_bind_vertex_buffers(*command_buffer, 0, &[vertex_buffer.instance], &[0]);
                device.instance.cmd_bind_index_buffer(*command_buffer, index_buffer.instance, 0, vk::IndexType::UINT16);
                device.instance.cmd_draw_indexed(*command_buffer, INDICES.len() as u32, 1, 0, 0, 0);
                device.instance.cmd_end_render_pass(*command_buffer);
                let _ = device.instance.end_command_buffer(*command_buffer);
            });

        Ok(buffers)
    }
}