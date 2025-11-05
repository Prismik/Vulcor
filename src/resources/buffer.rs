use ash::vk;
use anyhow::{anyhow, Result};

use crate::{cmd::command_pool::CmdPool, core::{context::VulkanContext, graphics::Graphics}, resources::image::Image};

pub struct Buffer {
    pub instance: vk::Buffer, 
    pub memory: vk::DeviceMemory,
    size: u64
}

impl Buffer {
    pub fn new(context: &VulkanContext, graphics: &Graphics, size: vk::DeviceSize, usage: vk::BufferUsageFlags, props: vk::MemoryPropertyFlags) -> Result<Self> {
        let mem = unsafe { context.instance.get_physical_device_memory_properties(graphics.physical.instance) };
        let create_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let buffer = unsafe { graphics.logical.instance.create_buffer(&create_info, None)? };

        let reqs = unsafe { graphics.logical.instance.get_buffer_memory_requirements(buffer) };
        let mem_info = vk::MemoryAllocateInfo::default()
            .allocation_size(reqs.size)
            .memory_type_index(Self::get_memory_type_index(mem, props, reqs)?);
        let buffer_mem = unsafe { graphics.logical.instance.allocate_memory(&mem_info, None)? };
        unsafe { graphics.logical.instance.bind_buffer_memory(buffer, buffer_mem, 0)? };

        Ok(Self { instance: buffer, memory: buffer_mem, size })
    }

    pub fn descriptor_buffer_info(&self) -> vk::DescriptorBufferInfo {
        vk::DescriptorBufferInfo::default()
            .buffer(self.instance)
            .offset(0)
            .range(self.size)
    }

    pub fn copy_to_img(&self, img: &Image, graphics: &Graphics, cmd_pool: &CmdPool) -> Result<()> {
        let command_buffer = graphics.begin_command_once(cmd_pool)?;
        let subresource = vk::ImageSubresourceLayers::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .mip_level(0)
            .base_array_layer(0)
            .layer_count(1);
        let region = vk::BufferImageCopy::default()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(subresource)
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0})
            .image_extent(vk::Extent3D {width: img.width, height: img.height, depth: 1 });

        unsafe { graphics.logical.instance.cmd_copy_buffer_to_image(command_buffer, self.instance, img.instance, vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region]) };
        graphics.end_command_once(cmd_pool, command_buffer)?;
        Ok(())
    }
    
    pub fn cleanup(&self, graphics: &Graphics) {
        unsafe {
            graphics.logical.instance.destroy_buffer(self.instance, None);
            graphics.logical.instance.free_memory(self.memory, None);
        }
    }

    // TODO Move into reusable functions in a base Resource class?
    fn get_memory_type_index(mem: vk::PhysicalDeviceMemoryProperties, props: vk::MemoryPropertyFlags, reqs: vk::MemoryRequirements) -> Result<u32> {
        (0..mem.memory_type_count)
            .find(|i| { 
                let suitable = (reqs.memory_type_bits & (1 << i)) != 0;
                let mem_type = mem.memory_types[*i as usize];
                suitable && mem_type.property_flags.contains(props)
            })
            .ok_or_else(|| anyhow!("No suitable memory type found."))
    }
}