use anyhow::{anyhow, Result};
use std::{collections::{BTreeMap, HashSet}, error::Error, ffi::CStr, fmt::{self, Display, Formatter}};
use ash::vk;

use crate::{core::context::VulkanContext, swapchain::SwapchainSupport};


pub struct QueueFamilyIndices {
    pub graphics: u32,
    pub presentation: u32
}

impl QueueFamilyIndices {
    pub fn new(context: &VulkanContext, physical_device: &vk::PhysicalDevice) -> Result<Self> {
        let properties = unsafe { context.instance.get_physical_device_queue_family_properties(*physical_device) };

        // TODO Unify both graphics and presentation queues
        let graphics = properties
            .iter()
            .position(|p| p.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .map(|i| i as u32);

        let mut presentation = None;
        for (index, _) in properties.iter().enumerate() {
            let supported = unsafe { context.surface_loader.get_physical_device_surface_support(*physical_device, index as u32, context.surface)? };
            if supported {
                presentation = Some(index as u32);
                break;
            }
        }
        
        if let (Some(graphics), Some(presentation)) = (graphics, presentation) {
            Ok(Self { graphics, presentation })
        } else {
            Err(anyhow!(PhysicalDeviceError::NoSuitableQueueFamily))
        }
    }

    pub fn unique_values(&self) -> HashSet<u32> {
        return HashSet::from([self.graphics, self.presentation]);
    }
}

#[derive(Debug)]
pub enum PhysicalDeviceError {
    NoSuitableDevice,
    NoSuitableQueueFamily
}

impl Display for PhysicalDeviceError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::NoSuitableDevice => write!(f, "No suitable physical devices found."),
            Self::NoSuitableQueueFamily => write!(f, "No suitable queue family found on the device."),
        }
    }
}

impl std::error::Error for PhysicalDeviceError {}

pub struct GraphicsHardware {
    pub instance: vk::PhysicalDevice
}

impl GraphicsHardware {
    pub fn new(context: &VulkanContext) -> Result<Self, Box<dyn Error>> {
        let physical_device = Self::select_physical_device(&context)?;
        Ok(Self { instance: physical_device })
    }

    pub fn required_extensions() -> Vec<&'static CStr> {
        let mut extensions = vec![ash::khr::swapchain::NAME];
        // Required by Vulkan SDK on macOS since 1.3.216.
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            extensions.push(ash::khr::portability_subset::NAME);
        }
        return extensions
    }

    fn select_physical_device(context: &VulkanContext) -> Result<vk::PhysicalDevice, Box<dyn Error>> {
        let devices = unsafe { context.instance.enumerate_physical_devices()? };
        let mut candidates: BTreeMap<i32, vk::PhysicalDevice> = BTreeMap::new();

        for physical_device in devices {
            let swapchain_support = SwapchainSupport::new(context, &physical_device)?;
            let score = Self::device_suitability_score(context, &physical_device, &swapchain_support);
            let properties = unsafe { context.instance.get_physical_device_properties(physical_device) };
            let name = unsafe { CStr::from_ptr(properties.device_name.as_ptr()) };
            println!("Physical device [{}] => {}", name.to_string_lossy(), score.to_string());
            candidates.insert(score, physical_device);
        }

        if let Some((&score, &physical_device)) = candidates.first_key_value() {
            if score > 0 {
                Ok(physical_device)
            } else {
                Err(Box::new(PhysicalDeviceError::NoSuitableDevice))
            }
        } else {
            Err(Box::new(PhysicalDeviceError::NoSuitableDevice))
        }
    }

        /// Assigns an increasing score based on the available features, or 0 when geometry shaders are not supported.
    fn device_suitability_score(context: &VulkanContext, physical_device: &vk::PhysicalDevice, swapchain: &SwapchainSupport) -> i32 {
        let queue_family = QueueFamilyIndices::new(context, physical_device);
        if queue_family.is_err() { return 0; }
        if !Self::device_supports_extensions(&context, physical_device) { return 0; }
    
        if swapchain.formats.is_empty() || swapchain.present_modes.is_empty() { return 0; }
    
        let properties = unsafe { context.instance.get_physical_device_properties(*physical_device) };
        let features = unsafe { context.instance.get_physical_device_features(*physical_device) };
        let mut score: i32 = 0;
        if features.geometry_shader == vk::FALSE { score += 2000; }
        if properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU { score += 1000; }

        score += properties.limits.max_image_dimension2_d as i32;
        return score;
    }

    fn device_supports_extensions(context: &VulkanContext, physical_device: &vk::PhysicalDevice) -> bool {
        let required: HashSet<&CStr> = Self::required_extensions().iter().map(|x| *x).collect::<HashSet<_>>();
        let properties = unsafe { context.instance.enumerate_device_extension_properties(*physical_device).unwrap() };
        let available = properties.iter()
            .map(|e| unsafe { CStr::from_ptr(e.extension_name.as_ptr()) })
            .collect::<HashSet<_>>();

        return required.intersection(&available).collect::<HashSet<_>>().len() == required.len();
    }
}