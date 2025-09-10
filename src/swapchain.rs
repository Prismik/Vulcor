use std::error::Error;
use ash::{khr::{surface, swapchain}, vk::{self, Extent2D, SwapchainKHR}, Device, Entry, Instance};
use winit::window::Window;

use crate::QueueFamilyIndices;

#[derive(Clone, Debug)]
pub struct SwapchainSupport {
    pub capabilities: vk::SurfaceCapabilitiesKHR,
    pub formats: Vec<vk::SurfaceFormatKHR>,
    pub present_modes: Vec<vk::PresentModeKHR>,
}

#[derive(Clone, Debug)]
pub struct SwapchainConfig {
    pub support: SwapchainSupport,
    pub capabilities: vk::SurfaceCapabilitiesKHR,
    pub format: vk::SurfaceFormatKHR,
    pub present_mode: vk::PresentModeKHR,
    pub extent: Extent2D
}

pub struct SwapchainData {
    pub khr: SwapchainKHR,
    pub loader: swapchain::Device,
    pub images: Vec<vk::Image>,
    pub image_views: Vec<vk::ImageView>,
    pub config: SwapchainConfig
}

impl SwapchainSupport {
    pub fn new(entry: &Entry, instance: &Instance, physical_device: &vk::PhysicalDevice, surface: &vk::SurfaceKHR) -> Result<Self, Box<dyn Error>> {
        let surface_loader = surface::Instance::new(entry, instance);
        let capabilities = unsafe { surface_loader.get_physical_device_surface_capabilities(*physical_device, *surface)? };
        let formats = unsafe { surface_loader.get_physical_device_surface_formats(*physical_device, *surface)? };
        let present_modes = unsafe { surface_loader.get_physical_device_surface_present_modes(*physical_device, *surface)? };

        Ok(Self {
            capabilities,
            formats,
            present_modes
        })
    }


}

impl SwapchainData {
    pub fn new(entry: &Entry, instance:&Instance, logical_device: &Device, physical_device: &vk::PhysicalDevice, surface: &vk::SurfaceKHR, window: &Window, surface_loader: &surface::Instance) -> Result<Self, Box<dyn Error>> {
        let loader = swapchain::Device::new(&instance, &logical_device);
        let (swapchain, config) = Self::create_swapchain(&entry, &instance, &physical_device, &surface, &window, &surface_loader, &loader)?;
        let images = unsafe { loader.get_swapchain_images(swapchain)? };
        let image_views = Self::create_image_views(&logical_device, &images, &config.format)?;
        Ok(Self {
            khr: swapchain,
            loader,
            images,
            image_views,
            config
        })
    }

    fn create_swapchain(entry: &Entry, instance: &Instance, physical_device: &vk::PhysicalDevice, surface: &vk::SurfaceKHR, window: &Window, surface_loader: &surface::Instance, swapchain_loader: &swapchain::Device) -> Result<(vk::SwapchainKHR, SwapchainConfig), Box<dyn Error>> {
        let queue_family = QueueFamilyIndices::new(physical_device, entry, instance, surface, surface_loader)?;
        let details = SwapchainSupport::new(entry, instance, physical_device, surface)?;
        let format = Self::select_swapchain_formats(&details);
        let present_mode = Self::select_swapchain_present_mode(&details);
        let extent = Self::select_swapchain_extent(&details, window);
        let image_count = {
            let max = details.capabilities.max_image_count;
            let preferred = details.capabilities.min_image_count + 1;
            if max > 0 { preferred.max(max) } else { preferred }
        };

        let use_concurrent_mode = queue_family.graphics != queue_family.presentation;
        let image_sharing_mode = if use_concurrent_mode { vk::SharingMode::CONCURRENT } else { vk::SharingMode::EXCLUSIVE };
        let queue_family_indices = if use_concurrent_mode { vec![queue_family.graphics, queue_family.presentation] } else { vec![] };
        let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(*surface)
            .min_image_count(image_count)
            .image_color_space(format.color_space)
            .image_format(format.format)
            .image_extent(extent)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(image_sharing_mode)
            .pre_transform(details.capabilities.current_transform)
            .present_mode(present_mode)
            .queue_family_indices(&queue_family_indices)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .clipped(true)
            .image_array_layers(1);

        let config = SwapchainConfig { capabilities: details.capabilities, format, present_mode, extent, support: details };
        let swapchain = unsafe { swapchain_loader.create_swapchain(&swapchain_create_info, None)? };
        Ok((swapchain, config))
    }

    fn select_swapchain_formats(support: &SwapchainSupport) -> vk::SurfaceFormatKHR  {
        *support.formats.iter()
            .find(|f| f.format == vk::Format::B8G8R8A8_SRGB && f.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .unwrap_or(&support.formats[0])
    }

    fn select_swapchain_present_mode(support: &SwapchainSupport) -> vk::PresentModeKHR {
        *support.present_modes.iter()
            .find(|&p| *p == vk::PresentModeKHR::MAILBOX)
            .unwrap_or(&vk::PresentModeKHR::FIFO)
    }

    fn select_swapchain_extent(support: &SwapchainSupport, window: &Window) -> vk::Extent2D {
        if support.capabilities.current_extent.width != std::u32::MAX {
            return support.capabilities.current_extent;
        }
        let min = support.capabilities.min_image_extent;
        let max = support.capabilities.max_image_extent;
        let width = window.inner_size().width.clamp(max.width, min.width);
        let height = window.inner_size().height.clamp(max.height, min.height);
        vk::Extent2D { width: width, height: height}
    }

    fn create_image_views(device: &Device, images: &Vec<vk::Image>, format: &vk::SurfaceFormatKHR) -> Result<Vec<vk::ImageView>, Box<dyn Error>> {
        let image_views = images.iter()
            .map(|img| {
                let info = vk::ImageViewCreateInfo::default()
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(format.format)
                    .components(vk::ComponentMapping {
                        r: vk::ComponentSwizzle::R,
                        g: vk::ComponentSwizzle::G,
                        b: vk::ComponentSwizzle::B,
                        a: vk::ComponentSwizzle::A,
                    })
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    })
                    .image(*img);
                unsafe { device.create_image_view(&info, None).unwrap() }
            })
            .collect::<Vec<_>>();

        Ok(image_views)
    }

}