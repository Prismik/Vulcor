use std::error::Error;

use ash::vk;

use crate::devices::Devices;

pub struct Semaphores {
    pub image_available: vk::Semaphore,
    pub render_completed: vk::Semaphore
}

impl Semaphores {
    pub fn new(devices: &Devices) -> Result<Self, Box<dyn Error>> {
        
        let image_available = {
            let semaphore_info = vk::SemaphoreCreateInfo::default();
            unsafe { devices.logical.create_semaphore(&semaphore_info, None)? }
        };
        let render_completed = {
            let semaphore_info = vk::SemaphoreCreateInfo::default();
            unsafe { devices.logical.create_semaphore(&semaphore_info, None)? }
        };

        Ok(Self{image_available, render_completed})
    }

    pub fn cleanup(&self, devices: &Devices) {
        unsafe { devices.logical.destroy_semaphore(self.image_available, None) };
        unsafe { devices.logical.destroy_semaphore(self.render_completed, None) };
    }
}