use std::{error::Error, ffi::{c_char, CStr}};
use ash::{ext::debug_utils, vk, Entry, Instance};
use raw_window_handle::RawDisplayHandle;

use crate::core::debug;

pub struct VulkanContext {
    pub entry: Entry,
    pub instance: Instance
}

impl VulkanContext {
    pub fn new(named: &CStr, handle: &RawDisplayHandle) -> Result<Self, Box<dyn Error>> {
        let entry = Entry::linked();
        let app_info = vk::ApplicationInfo::default()
            .application_name(named)
            .application_version(0)
            .engine_name(named)
            .engine_version(0)
            .api_version(vk::API_VERSION_1_3);

        let mut extension_names = ash_window::enumerate_required_extensions(*handle)
            .unwrap()
            .to_vec();
        
        extension_names.push(debug_utils::NAME.as_ptr());
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            extension_names.push(ash::khr::portability_enumeration::NAME.as_ptr());
            // Enabling this extension is a requirement when using `VK_KHR_portability_subset`
            extension_names.push(ash::khr::get_physical_device_properties2::NAME.as_ptr());
        }

        let flags = if cfg!(any(target_os = "macos", target_os = "ios")) {
            vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
        } else {
            vk::InstanceCreateFlags::default()
        };

        let mut info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            //.enabled_layer_names(&enabled_layer_names)
            .enabled_extension_names(&extension_names)
            .flags(flags);

        let layers_names_raw: Vec<*const c_char> = debug::VALIDATION_LAYERS
            .iter()
            .map(|raw_name| raw_name.as_ptr())
            .collect();

        let mut debug_info = debug::create_debug_info();
        if debug::VALIDATION_ENABLED {
            if debug::validation_layers_supported(&entry) {
                info = info.enabled_layer_names(&layers_names_raw)
                    .push_next(&mut debug_info);
            } else {
                panic!("Validation layers not supported")
            }
        }

        let instance = unsafe { entry.create_instance(&info, None)? };
        Ok(Self{entry, instance})
    }
}