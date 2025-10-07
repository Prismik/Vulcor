use ash::{vk, Device};
use anyhow::{Result};

use crate::{core::{context::VulkanContext, physical_device::GraphicsHardware}, QueueFamilyIndices};

pub struct GraphicsInterface {
    pub instance: Device
}

impl GraphicsInterface {
    pub fn new(context: &VulkanContext, physical_device: &GraphicsHardware, queue_family: &QueueFamilyIndices) -> Result<GraphicsInterface> {
        let queue_priority = &[1.0];
        let queue_create_infos = queue_family.unique_values().iter().map(|family_index|
            vk::DeviceQueueCreateInfo::default()
                .queue_family_index(*family_index)
                .queue_priorities(queue_priority)
        ).collect::<Vec<_>>();

        let features = vk::PhysicalDeviceFeatures::default();
        let extensions = GraphicsHardware::required_extensions().into_iter().map(|e| e.as_ptr()).collect::<Vec<_>>();
        let device_create_info: vk::DeviceCreateInfo<'_> = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_features(&features)
            .enabled_extension_names(&extensions);

        let device = unsafe { context.instance.create_device(physical_device.instance, &device_create_info, None)? };
        Ok(Self { instance: device })
    }
}