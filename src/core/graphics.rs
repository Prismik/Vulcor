use std::sync::Arc;

use anyhow::{anyhow, Result};
use ash::vk::{self, SubmitInfo};

use crate::{cmd::command_pool::CmdPool, core::{context::VulkanContext, logical_device::GraphicsInterface, physical_device::{GraphicsHardware, QueueFamilyIndices}}, pipeline::render_pipeline::{Vertex, VERTICES}};


pub struct Graphics {
    pub physical: GraphicsHardware,
    pub logical: GraphicsInterface,
    pub queue: vk::Queue
}

impl Graphics {
    pub fn new(context: &VulkanContext) -> Result<Self> {
        let physical = GraphicsHardware::new(context)?;
        let queue_family = QueueFamilyIndices::new(context, &physical.instance)?;
        let logical = GraphicsInterface::new(context, &physical, &queue_family)?;
        let graphics_queue = unsafe { logical.instance.get_device_queue(queue_family.graphics, 0) };
        
        Ok(Self { physical: physical, logical: logical, queue: graphics_queue })
    }

    pub unsafe fn copy_buffer(&self, src: &vk::Buffer, dst: &vk::Buffer, size: vk::DeviceSize, cmd_pool: &CmdPool) -> Result<()> {
        let allocate_info = vk::CommandBufferAllocateInfo::default()
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_pool(cmd_pool.instance)
            .command_buffer_count(1);

        let command_buffer = self.logical.instance.allocate_command_buffers(&allocate_info)?[0];

        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        self.logical.instance.begin_command_buffer(command_buffer, &begin_info)?; // Begin
        let regions = vk::BufferCopy::default().size(size);
        self.logical.instance.cmd_copy_buffer(command_buffer, *src, *dst, &[regions]);
        self.logical.instance.end_command_buffer(command_buffer)?; // End

        let command_buffers = &[command_buffer];
        let submit_info = vk::SubmitInfo::default()
            .command_buffers(command_buffers);

        self.queue_submit(&vec![submit_info], vk::Fence::null())?;
        self.logical.instance.queue_wait_idle(self.queue)?;
        self.logical.instance.free_command_buffers(cmd_pool.instance, command_buffers);
        Ok(())
    }

    pub fn queue_submit(&self, submits: &Vec<SubmitInfo>, fence: vk::Fence) -> Result<()> {
        unsafe { self.logical.instance.queue_submit(self.queue, submits, fence)? };
        Ok(())
    }
}
