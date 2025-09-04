use ash::{vk, Entry, Instance, ext::debug_utils};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    error::Error, ffi::{c_char, CStr, CString}, result::Result
};
use log::{info, error};
use winit::{
    application::ApplicationHandler, dpi::LogicalSize, event::{self, Event, WindowEvent}, event_loop::{ActiveEventLoop, EventLoop}, window::{Window, WindowAttributes, WindowId}
};

const VALIDATION_ENABLED: bool = cfg!(debug_assertions);
const VALIDATION_LAYERS: [&'static CStr; 1] = [c"VK_LAYER_KHRONOS_validation"];

struct Vulcor {
    name: String,
    window: Option<Window>,
    entry: Option<Entry>,
    instance: Option<Instance>
}

impl Vulcor {
    fn new() -> Result<Self, Box<dyn Error>> {
        info!("Creating application");
        let title = "Vulcor";
        Ok(Self{
            name: title.to_string(),  
            window: None,
            entry: None,
            instance: None
        })
    }

    fn is_initialized(&self) -> bool {
        return self.entry.is_some() && self.instance.is_some();
    }

    fn init(&mut self) -> Result<(), Box<dyn Error>> {
        let entry = unsafe { ash::Entry::load().expect("Failed to create entry") };
        self.entry = Some(entry);
        let instance = self.create_instance(CString::new(self.name.as_str())?.as_c_str())?;
        self.instance = Some(instance);
        Ok(())
    }

    fn create_instance(&self, named: &CStr) -> Result<Instance, Box<dyn Error>> {
        let app_info = vk::ApplicationInfo::default()
            .application_name(named)
            .application_version(0)
            .engine_name(named)
            .engine_version(0)
            .api_version(vk::API_VERSION_1_3);

        let handle = self.window.as_ref().unwrap().display_handle()?.as_raw();
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

        if VALIDATION_ENABLED {
            if self.validation_layers_supported() {
                info = info.enabled_layer_names(&layers_names_raw);
            } else {
                panic!("Validation layers not supported")
            }
        }

        unsafe { Ok(self.entry.as_ref().unwrap().create_instance(&info, None)?) }
    }

    fn validation_layers_supported(&self) -> bool {
        let mut found: bool = true;
        for required in VALIDATION_LAYERS.iter() {
            found = unsafe {
                self.entry.as_ref().unwrap()
                    .enumerate_instance_layer_properties()
                    .unwrap()
                    .iter()
                    .any(|layer| {
                        let name = CStr::from_ptr(layer.layer_name.as_ptr());
                        required == &name
                    })
            };

            if !found {
                break;
            }
        }

        return found
    }

    fn render(&self) {
        println!("render()");
    }

    unsafe fn cleanup(&mut self) {
        self.instance.take().unwrap().destroy_instance(None);
        self.entry.take();
    }
}

impl ApplicationHandler for Vulcor {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes().with_title(self.name.as_str());
        self.window = Some(event_loop.create_window(window_attributes).unwrap());
        
        if !self.is_initialized() {
            let _ = self.init();
            println!("Instance initialized")
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        // println!("{event:?}");
        match event {
            WindowEvent::RedrawRequested => self.render(),
            WindowEvent::CloseRequested => {
                unsafe { self.cleanup() };
                event_loop.exit()
            },
            _ => (),
        }
    }
}

impl Drop for Vulcor {
    fn drop(&mut self) {
        unsafe { self.cleanup() };
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut app = Vulcor::new()?;
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;

    Ok(())
}
