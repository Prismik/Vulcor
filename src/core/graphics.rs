use anyhow::{anyhow, Result};
use ash::vk;
use std::{ptr::copy_nonoverlapping as memcpy};

use crate::{core::{context::VulkanContext, logical_device::GraphicsInterface, physical_device::GraphicsHardware}, pipeline::render_pipeline::{Vertex, VERTICES}};


pub struct Devices {
    pub physical: GraphicsHardware,
    pub logical: GraphicsInterface,
}

impl Devices {
    pub fn new(context: &VulkanContext) -> Result<Self> {
        let physical = GraphicsHardware::new(context)?;
        let logical = GraphicsInterface::new(context, &physical)?;
        Ok(Self { physical, logical: logical })
    }

    pub unsafe fn create_vertex_buffer(&self, context: &VulkanContext) -> Result<(vk::Buffer, vk::DeviceMemory)> {
        let mem = context.instance.get_physical_device_memory_properties(self.physical.instance);
        
        let create_info = vk::BufferCreateInfo::default()
            .size((size_of::<Vertex>() * VERTICES.len()) as u64)
            .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let vertex_buffer = self.logical.instance.create_buffer(&create_info, None)?;

        let reqs = self.logical.instance.get_buffer_memory_requirements(vertex_buffer);
        let mem_info = vk::MemoryAllocateInfo::default()
            .allocation_size(reqs.size)
            .memory_type_index(self.get_memory_type_index(mem, vk::MemoryPropertyFlags::HOST_COHERENT | vk::MemoryPropertyFlags::HOST_VISIBLE, reqs)?);
        let vertex_buffer_mem = self.logical.instance.allocate_memory(&mem_info, None)?;
        self.logical.instance.bind_buffer_memory(vertex_buffer, vertex_buffer_mem, 0)?;
        let mem = self.logical.instance.map_memory(vertex_buffer_mem, 0, create_info.size, vk::MemoryMapFlags::empty())?;
        memcpy(VERTICES.as_ptr(), mem.cast(), VERTICES.len());
        self.logical.instance.unmap_memory(vertex_buffer_mem);
        Ok((vertex_buffer, vertex_buffer_mem))
    }

    fn get_memory_type_index(&self, mem: vk::PhysicalDeviceMemoryProperties, props: vk::MemoryPropertyFlags, reqs: vk::MemoryRequirements) -> Result<u32> {
        (0..mem.memory_type_count)
            .find(|i| { 
                let suitable = (reqs.memory_type_bits & (1 << i)) != 0;
                let mem_type = mem.memory_types[*i as usize];
                suitable && mem_type.property_flags.contains(props)
            })
            .ok_or_else(|| anyhow!("No suitable memory type found."))
    }
}
