use std::{collections::{BTreeMap, HashSet}, error::Error, ffi::CStr, fmt::{self, Display, Formatter}};
use ash::{khr::{surface}, vk, Device, Entry, Instance};

use crate::{swapchain::SwapchainSupport, QueueFamilyIndices};

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

pub struct Devices {
    pub physical: vk::PhysicalDevice,
    pub logical: Device,
}

impl Devices {
    pub fn new(entry: &Entry, instance: &Instance, surface: &vk::SurfaceKHR, surface_loader: &surface::Instance) -> Result<Self, Box<dyn Error>> {
        let physical_device = Self::select_physical_device(&entry, &instance, &surface, &surface_loader)?;
        let logical_device = Self::create_logical_device(&physical_device, &entry, &instance, &surface, &surface_loader)?;
        Ok(Self { physical: physical_device, logical: logical_device })
    }

    fn select_physical_device(entry: &Entry, instance: &Instance, surface: &vk::SurfaceKHR, surface_loader: &surface::Instance) -> Result<vk::PhysicalDevice, Box<dyn Error>> {
        let devices = unsafe { instance.enumerate_physical_devices()? };
        let mut candidates: BTreeMap<i32, vk::PhysicalDevice> = BTreeMap::new();

        for physical_device in devices {
            let swapchain_support = SwapchainSupport::new(entry, instance, &physical_device, surface)?;
            let score = Self::device_suitability_score(&physical_device, entry, instance, surface, surface_loader, &swapchain_support);
            let properties = unsafe { instance.get_physical_device_properties(physical_device) };
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

    fn create_logical_device(physical_device: &vk::PhysicalDevice, entry: &Entry, instance: &Instance, surface: &vk::SurfaceKHR, surface_loader: &surface::Instance) -> Result<Device, Box<dyn Error>> {
        let queue_family = QueueFamilyIndices::new(physical_device, entry, instance, surface, surface_loader)?;
        let queue_priority = &[1.0];
        let queue_create_infos = queue_family.unique_values().iter().map(|family_index|
            vk::DeviceQueueCreateInfo::default()
                .queue_family_index(*family_index)
                .queue_priorities(queue_priority)
        ).collect::<Vec<_>>();

        let features = vk::PhysicalDeviceFeatures::default();
        let extensions = Self::required_extensions().into_iter().map(|e| e.as_ptr()).collect::<Vec<_>>();
        let device_create_info: vk::DeviceCreateInfo<'_> = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_features(&features)
            .enabled_extension_names(&extensions);

        let device = unsafe { instance.create_device(*physical_device, &device_create_info, None)? };
        Ok(device)
    }

    /// Assigns an increasing score based on the available features, or 0 when geometry shaders are not supported.
    fn device_suitability_score(physical_device: &vk::PhysicalDevice, entry: &Entry, instance: &Instance, surface: &vk::SurfaceKHR, surface_loader: &surface::Instance, swapchain: &SwapchainSupport) -> i32 {
        let queue_family = QueueFamilyIndices::new(physical_device, entry, instance, surface, surface_loader);
        if queue_family.is_err() { return 0; }
        if !Self::device_supports_extensions(physical_device, instance) { return 0; }
    
        if swapchain.formats.is_empty() || swapchain.present_modes.is_empty() { return 0; }
    
        let properties = unsafe { instance.get_physical_device_properties(*physical_device) };
        let features = unsafe { instance.get_physical_device_features(*physical_device) };
        let mut score: i32 = 0;
        if features.geometry_shader == vk::FALSE { score += 2000; }
        if properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU { score += 1000; }

        score += properties.limits.max_image_dimension2_d as i32;
        return score;
    }

    fn device_supports_extensions(physical_device: &vk::PhysicalDevice, instance: &Instance) -> bool {
        let required: HashSet<&CStr> = Self::required_extensions().iter().map(|x| *x).collect::<HashSet<_>>();
        let properties = unsafe { instance.enumerate_device_extension_properties(*physical_device).unwrap() };
        let available = properties.iter()
            .map(|e| unsafe { CStr::from_ptr(e.extension_name.as_ptr()) })
            .collect::<HashSet<_>>();

        return required.intersection(&available).collect::<HashSet<_>>().len() == required.len();
    }

    fn required_extensions() -> Vec<&'static CStr> {
        let mut extensions = vec![ash::khr::swapchain::NAME];
        // Required by Vulkan SDK on macOS since 1.3.216.
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            extensions.push(ash::khr::portability_subset::NAME);
        }
        return extensions
    }
}