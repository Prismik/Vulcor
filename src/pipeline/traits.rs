use std::error::Error;

use ash::{vk, Device};

use crate::swapchain::SwapchainConfig;

pub trait VulkanPipeline {
    fn new(logical_device: &Device, config: &SwapchainConfig, render_pass: &vk::RenderPass) -> Result<Self, Box<dyn Error>> where Self: Sized;
    fn instance(&self) -> vk::Pipeline;
    fn layout(&self) -> vk::PipelineLayout;
    fn cleanup(&self, device: &Device);
}