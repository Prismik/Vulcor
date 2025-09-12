use std::{collections::{BTreeMap, HashSet}, error::Error, ffi::CStr, fmt::{self, Display, Formatter}};
use ash::{vk, Device};

use crate::{core::{context::VulkanContext, physical_device::{self, GraphicsHardware}}, swapchain::SwapchainSupport, QueueFamilyIndices};

pub struct Devices {
    pub physical: vk::PhysicalDevice,
    pub logical: Device,
}

impl Devices {
    pub fn new(context: &VulkanContext) -> Result<Self, Box<dyn Error>> {
        let physical_device = GraphicsHardware::new(context)?;
        let logical_device = Self::create_logical_device(context, &physical_device.instance)?;
        Ok(Self { physical: physical_device.instance, logical: logical_device })
    }

    fn create_logical_device(context: &VulkanContext, physical_device: &vk::PhysicalDevice) -> Result<Device, Box<dyn Error>> {
        let queue_family = QueueFamilyIndices::new(context, physical_device)?;
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

        let device = unsafe { context.instance.create_device(*physical_device, &device_create_info, None)? };
        Ok(device)
    }
}