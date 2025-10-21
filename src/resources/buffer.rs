use ash::vk;
use anyhow::{anyhow, Result};

use crate::{core::{context::VulkanContext, graphics::Graphics}};

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

    pub fn cleanup(&self, graphics: &Graphics) {
        unsafe {
            graphics.logical.instance.destroy_buffer(self.instance, None);
            graphics.logical.instance.free_memory(self.memory, None);
        }
    }

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