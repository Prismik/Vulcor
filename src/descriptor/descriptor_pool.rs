use anyhow::{Result};
use std::any::Any;

use ash::vk::{self, DescriptorSet};

use crate::{core::graphics::Graphics, resources::buffer::Buffer};

pub struct DescriptorPool {
    pub instance: vk::DescriptorPool,
    pub sets: Vec<vk::DescriptorSet>,
    pub layout: vk::DescriptorSetLayout
}

impl DescriptorPool {
    pub fn new(size: u32, graphics: &Graphics, uniform_buffers: &Vec<Buffer>) -> Result<Self> {
        let pool_size = vk::DescriptorPoolSize::default()
            .ty(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(size);
        let pool_sizes = &[pool_size];
        let create_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(pool_sizes)
            .max_sets(size);

        let pool = unsafe { graphics.logical.instance.create_descriptor_pool(&create_info, None)? };
        let layout = Self::create_descriptor_set_layout(&graphics)?;
        let layouts = vec![layout; size as usize];
        let allocate_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(pool)
            .set_layouts(&layouts);
        let sets = unsafe { graphics.logical.instance.allocate_descriptor_sets(&allocate_info)? };
        Self::configure_descriptor_sets(&sets, uniform_buffers, graphics);
        Ok(Self { instance: pool, sets: sets, layout })
    }

    pub fn cleanup(&self, graphics: &Graphics) {
        unsafe {
            graphics.logical.instance.destroy_descriptor_pool(self.instance, None);
        }
    }

    fn configure_descriptor_sets(sets: &Vec<DescriptorSet>, uniform_buffers: &Vec<Buffer>, graphics: &Graphics) {
        for i in 0..uniform_buffers.len() {
            let info = uniform_buffers[i].descriptor_buffer_info();
            let buffer_info = &[info];
            let buffer_write = vk::WriteDescriptorSet::default()
                .dst_set(sets[i])
                .dst_binding(0)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(buffer_info);

            unsafe { graphics.logical.instance.update_descriptor_sets(&[buffer_write], &[] as &[vk::CopyDescriptorSet]) };
        }
    }

    fn create_descriptor_set_layout(graphics: &Graphics) -> Result<vk::DescriptorSetLayout> {
        let binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX);

        let bindings = &[binding];
        let create_info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(bindings);
        let layout = unsafe { graphics.logical.instance.create_descriptor_set_layout(&create_info, None)? };
        Ok(layout)
    }
}