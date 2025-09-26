use anyhow::{Result};
use ash::vk;
use crate::{Devices, swapchain::SwapchainData};

const MAX_FRAMES_IN_FLIGHT: usize = 2;

pub struct RenderSync {
    image_available: Vec<vk::Semaphore>,
    render_completed: Vec<vk::Semaphore>,
    in_flight: Vec<vk::Fence>,
    pub images_in_flight: Vec<vk::Fence>,
    frame: usize,
}

impl RenderSync {
    pub fn new(devices: &Devices, swapchain: &SwapchainData) -> Result<Self> {
        let mut image_available_semaphores: Vec<vk::Semaphore> = vec![];
        let mut render_completed_semaphores: Vec<vk::Semaphore> = vec![];
        let mut in_flight_fences: Vec<vk::Fence> = vec![];
        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            let image_available = {
                let create_info = vk::SemaphoreCreateInfo::default();
                unsafe { devices.logical.create_semaphore(&create_info, None)? }
            };
            image_available_semaphores.push(image_available);
            let render_completed = {
                let create_info = vk::SemaphoreCreateInfo::default();
                unsafe { devices.logical.create_semaphore(&create_info, None)? }
            };
            render_completed_semaphores.push(render_completed);
            let fence = {
                let create_info = vk::FenceCreateInfo::default()
                    .flags(vk::FenceCreateFlags::SIGNALED);
                unsafe { devices.logical.create_fence(&create_info, None)? }
            };
            in_flight_fences.push(fence);
        }

        let images_in_flight = swapchain.images.iter()
            .map(|_| vk::Fence::null())
            .collect();

        Ok(Self{
            image_available: image_available_semaphores, 
            render_completed: render_completed_semaphores,
            in_flight: in_flight_fences,
            images_in_flight,
            frame: 0
        })
    }

    pub fn cleanup(&self, devices: &Devices) {
        self.image_available.iter().for_each(|s| {
            unsafe { devices.logical.destroy_semaphore(*s, None) };
        });
        self.render_completed.iter().for_each(|s| {
            unsafe { devices.logical.destroy_semaphore(*s, None) };
        });
        self.in_flight.iter().for_each(|f| {
            unsafe { devices.logical.destroy_fence(*f, None) };
        });
        self.images_in_flight.iter().for_each(|f| {
            unsafe { devices.logical.destroy_fence(*f, None) };
        });
    }

    pub fn get_image_available(&self) -> vk::Semaphore {
        self.image_available[self.frame]
    }

    pub fn get_render_completed(&self) -> vk::Semaphore {
        self.render_completed[self.frame]
    }

    pub fn get_in_flight_fence(&self) -> vk::Fence {
        self.in_flight[self.frame]
    }

    pub fn increment_frame(&mut self) {
        self.frame = (self.frame + 1) % MAX_FRAMES_IN_FLIGHT;
    }

    pub fn update_image_in_flight(&mut self, index: usize) {
        self.images_in_flight[index] = self.get_in_flight_fence();
    }

    pub fn reset_fences(&self, devices: &Devices) -> Result<()> {
        unsafe { devices.logical.reset_fences(&[self.get_in_flight_fence()])? };
        Ok(())
    }
}