use ash::vk;
use anyhow::{anyhow, Result};

use crate::{core::{context::VulkanContext, graphics::Graphics}};

pub struct Image {
    pub instance: vk::Image, 
    pub memory: vk::DeviceMemory,
    size: u64
}

impl Image {
    pub fn new(
        context: &VulkanContext, 
        graphics: &Graphics, 
        extent: (u32, u32), 
        size: vk::DeviceSize, 
        usage: vk::ImageUsageFlags, 
        props: vk::MemoryPropertyFlags,
        format: vk::Format,
        tiling: vk::ImageTiling
    ) -> Result<Self> {
        let mem = unsafe { context.instance.get_physical_device_memory_properties(graphics.physical.instance) };
        let info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .extent(vk::Extent3D { width: extent.1, height: extent.1, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .format(format)
            .tiling(tiling)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .samples(vk::SampleCountFlags::TYPE_1)
            .flags(vk::ImageCreateFlags::empty());
        let img = unsafe { graphics.logical.instance.create_image(&info, None)? };
        
        let reqs = unsafe { graphics.logical.instance.get_image_memory_requirements(img) };
        let mem_info = vk::MemoryAllocateInfo::default()
            .allocation_size(reqs.size)
            .memory_type_index(Self::get_memory_type_index(mem, props, reqs)?);
        let img_mem = unsafe { graphics.logical.instance.allocate_memory(&mem_info, None)? };
        unsafe { graphics.logical.instance.bind_image_memory(img, img_mem, 0)? };

        Ok(Self { instance: img, memory: img_mem, size })
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