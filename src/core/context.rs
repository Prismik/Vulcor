use std::{error::Error, ffi::{c_char, CStr}};
use ash::{ext::debug_utils, khr::surface, vk, Entry, Instance};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::Window;

use crate::core::debug;

pub struct VulkanContext {
    pub entry: Entry,
    pub instance: Instance,
    pub surface: vk::SurfaceKHR,
    pub surface_loader: surface::Instance,
}

impl VulkanContext {
    pub fn new(named: &CStr, window: &Window) -> Result<Self, Box<dyn Error>> {
        let entry = Entry::linked();
        let app_info = vk::ApplicationInfo::default()
            .application_name(named)
            .application_version(0)
            .engine_name(named)
            .engine_version(0)
            .api_version(vk::API_VERSION_1_3);

        let display_handle = window.display_handle()?.as_raw();
        let mut extension_names = ash_window::enumerate_required_extensions(display_handle)
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
        let surface = unsafe { ash_window::create_surface(
            &entry, 
            &instance, 
            display_handle, 
            window.window_handle()?.as_raw(), 
            None
        )? };
        let surface_loader = surface::Instance::new(&entry, &instance);
        Ok(Self{entry, instance, surface, surface_loader})
    }

    pub fn cleanup(&self) {
        unsafe {
            self.surface_loader.destroy_surface(self.surface, None);
            self.instance.destroy_instance(None);
        }
    }
}