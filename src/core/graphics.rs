use anyhow::{anyhow, Result};
use ash::vk::{self, SubmitInfo};

use crate::{cmd::command_pool::CmdPool, core::{context::VulkanContext, logical_device::GraphicsInterface, physical_device::{GraphicsHardware, QueueFamilyIndices}}, pipeline::render_pipeline::{Vertex, VERTICES}, resources::image::Image};


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
        let command_buffer = self.begin_command_once(cmd_pool)?; // Begin
        let regions = vk::BufferCopy::default().size(size);
        self.logical.instance.cmd_copy_buffer(command_buffer, *src, *dst, &[regions]);
        self.end_command_once(cmd_pool, command_buffer)?;
        Ok(())
    }

    pub fn queue_submit(&self, submits: &Vec<SubmitInfo>, fence: vk::Fence) -> Result<()> {
        unsafe { self.logical.instance.queue_submit(self.queue, submits, fence)? };
        Ok(())
    }

    pub fn begin_command_once(&self, cmd_pool: &CmdPool) -> Result<vk::CommandBuffer> {
        let allocate_info = vk::CommandBufferAllocateInfo::default()
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_pool(cmd_pool.instance)
            .command_buffer_count(1);

        let command_buffer = unsafe { self.logical.instance.allocate_command_buffers(&allocate_info)?[0] };
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        unsafe { self.logical.instance.begin_command_buffer(command_buffer, &begin_info)?; }
        Ok(command_buffer)
    }

    pub fn end_command_once(&self, cmd_pool: &CmdPool, command_buffer: vk::CommandBuffer) -> Result<()> {
        unsafe { self.logical.instance.end_command_buffer(command_buffer)?; } // End
        let command_buffers = &[command_buffer];
        let submit_info = vk::SubmitInfo::default()
            .command_buffers(command_buffers);
        self.queue_submit(&vec![submit_info], vk::Fence::null())?;
        unsafe {
            self.logical.instance.queue_wait_idle(self.queue)?;
            self.logical.instance.free_command_buffers(cmd_pool.instance, command_buffers);
        }
        Ok(())
    }

    pub fn transition_img_layout(&self, cmd_pool: &CmdPool, image: Image, format: vk::Format, old: vk::ImageLayout, new: vk::ImageLayout) -> Result<()> {
        let command_buffer = self.begin_command_once(cmd_pool)?;
        let subresource = vk::ImageSubresourceRange::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .base_mip_level(0)
            .level_count(1)
            .base_array_layer(0)
            .layer_count(1);
        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(old)
            .new_layout(new)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image.instance)
            .subresource_range(subresource)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::empty());
        unsafe {
            self.logical.instance.cmd_pipeline_barrier(
                command_buffer, 
                vk::PipelineStageFlags::empty(), 
                vk::PipelineStageFlags::empty(), 
                vk::DependencyFlags::empty(), 
                &[] as &[vk::MemoryBarrier], 
                &[] as &[vk::BufferMemoryBarrier], 
                &[barrier]
            );
        }
        self.end_command_once(cmd_pool, command_buffer)?;
        Ok(())
    }
}
