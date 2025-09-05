mod debug;

use ash::{vk, Entry, Instance, ext::debug_utils, ext::debug_report};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    error::Error, ffi::{c_char, c_void, CStr, CString}, result::Result
};
use log::{debug, info, error, warn, trace};
use winit::{
    application::ApplicationHandler, dpi::LogicalSize, event::{self, Event, WindowEvent}, event_loop::{ActiveEventLoop, EventLoop}, window::{Window, WindowAttributes, WindowId}
};

const VALIDATION_ENABLED: bool = cfg!(debug_assertions);
const VALIDATION_LAYERS: [&'static CStr; 1] = [c"VK_LAYER_KHRONOS_validation"];

struct App {
    name: String,
    vulcor: Option<Vulcor>
}

impl App {
    fn new() -> App {
        Self { name: "Vulcor".to_string(), vulcor: None }
    }
}

struct Vulcor {
    name: String,
    window: Window,
    entry: Entry,
    instance: Instance,
    messenger: Option<(debug_utils::Instance, vk::DebugUtilsMessengerEXT)>
}

impl Vulcor {
    fn new(window: Window) -> Result<Self, Box<dyn Error>> {
        info!("Creating application");
        let title = "Vulcor";
        let entry = unsafe { ash::Entry::load().expect("Failed to create entry") };
        let instance = Self::create_instance(CString::new(title)?.as_c_str(), &entry, &window)?;
        let messenger = debug::setup_debug_messenger(&entry, &instance);
        Ok(Self{
            name: title.to_string(),  
            window,
            entry,
            instance,
            messenger
        })
    }


    fn create_instance(named: &CStr, entry: &Entry, window: &Window) -> Result<Instance, Box<dyn Error>> {
        let app_info = vk::ApplicationInfo::default()
            .application_name(named)
            .application_version(0)
            .engine_name(named)
            .engine_version(0)
            .api_version(vk::API_VERSION_1_3);

        let handle = window.display_handle()?.as_raw();
        let mut extension_names = ash_window::enumerate_required_extensions(handle)
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

        let layers_names_raw: Vec<*const c_char> = VALIDATION_LAYERS
            .iter()
            .map(|raw_name| raw_name.as_ptr())
            .collect();

        let mut debug_info = debug::create_debug_info();
        if VALIDATION_ENABLED {
            if debug::validation_layers_supported(entry) {
                info = info.enabled_layer_names(&layers_names_raw)
                    .push_next(&mut debug_info);
            } else {
                panic!("Validation layers not supported")
            }
        }

        unsafe { Ok(entry.create_instance(&info, None)?) }
    }

    fn render(&self) {
        
    }

    fn cleanup(&self) {
        if let Some((report, callback)) = self.messenger.as_ref().take() {
            unsafe { report.destroy_debug_utils_messenger(*callback, None) };
        }
        unsafe { self.instance.destroy_instance(None) };
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        match self.vulcor {
            None => { 
                let window_attributes = Window::default_attributes().with_title(self.name.as_str());
                let window = event_loop.create_window(window_attributes).unwrap();
                self.vulcor = match Vulcor::new(window) {
                    Ok(vulcor) => Some(vulcor),
                    Err(error) => panic!("Unable to create Vulcor object {}", error)
                };
            },
            _ => ()
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        // println!("{event:?}");
        match self.vulcor.as_ref() {
            Some(instance) => {
                match event {
                    WindowEvent::RedrawRequested => instance.render(),
                    WindowEvent::CloseRequested => {
                        instance.cleanup();
                        event_loop.exit()
                    },
                    _ => (),
                }
            }
            _ => ()
        }

    }
}

impl Drop for Vulcor {
    fn drop(&mut self) {
        self.cleanup();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut app = App::new();
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;

    Ok(())
}
