mod debug;

use ash::{vk, Entry, Instance, Device, ext::debug_utils, ext::debug_report};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    collections::{BTreeMap, HashMap}, error::Error, ffi::{c_char, c_void, CStr, CString}, fmt::{self, Display, Formatter}, hash::Hash, result::Result
};
use log::{debug, info, error, warn, trace};
use winit::{
    application::ApplicationHandler, dpi::LogicalSize, event::{self, Event, WindowEvent}, event_loop::{ActiveEventLoop, EventLoop}, window::{Window, WindowAttributes, WindowId}
};

#[derive(Debug)]
enum PhysicalDeviceError {
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

struct QueueFamilyIndices {
    graphics: u32,
}

impl QueueFamilyIndices {
    fn new(physical_device: &vk::PhysicalDevice, instance: &Instance) -> Result<Self, Box<dyn Error>> {
        let properties = unsafe { instance.get_physical_device_queue_family_properties(*physical_device) };
        let graphics = properties
            .iter()
            .position(|p| p.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .map(|i| i as u32);

        if let Some(graphics) = graphics {
            Ok(Self { graphics })
        } else {
            Err(Box::new(PhysicalDeviceError::NoSuitableQueueFamily))
        }
    }
}

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
    messenger: Option<(debug_utils::Instance, vk::DebugUtilsMessengerEXT)>,
    physical_device: vk::PhysicalDevice,
    logical_device: Device,
    graphics_queue: vk::Queue
}

impl Vulcor {
    fn new(window: Window) -> Result<Self, Box<dyn Error>> {
        info!("Creating application");
        let title = "Vulcor";
        let entry = Entry::linked();
        let instance = Self::create_instance(CString::new(title)?.as_c_str(), &entry, &window)?;
        let messenger = debug::setup_debug_messenger(&entry, &instance);
        let physical_device = Self::select_physical_device(&instance)?;
        let logical_device = Self::create_logical_device(&physical_device, &entry, &instance)?;
        let queue_family = QueueFamilyIndices::new(&physical_device, &instance)?;
        let graphics_queue = unsafe { logical_device.get_device_queue(queue_family.graphics, 0) };
        Ok(Self{
            name: title.to_string(),  
            window,
            entry,
            instance,
            messenger,
            physical_device,
            logical_device,
            graphics_queue
        })
    }

    fn select_physical_device(instance: &Instance) -> Result<vk::PhysicalDevice, Box<dyn Error>> {
        let devices = unsafe { instance.enumerate_physical_devices()? };
        let mut candidates: BTreeMap<i32, vk::PhysicalDevice> = BTreeMap::new();

        for physical_device in devices {
            let score = Self::device_suitability_score(&physical_device, instance);
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

    fn create_logical_device(physical_device: &vk::PhysicalDevice, entry: &Entry, instance: &Instance) -> Result<Device, Box<dyn Error>> {
        let queue_family = QueueFamilyIndices::new(physical_device, instance)?;
        let queue_create_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family.graphics)
            .queue_priorities(&[1.0]);

        let features = vk::PhysicalDeviceFeatures::default();
        let queue_infos = &[queue_create_info];
        let mut extensions = vec![];
        // Required by Vulkan SDK on macOS since 1.3.216.
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            extensions.push(ash::khr::portability_subset::NAME.as_ptr());
        }
        let device_create_info: vk::DeviceCreateInfo<'_> = vk::DeviceCreateInfo::default()
            .queue_create_infos(queue_infos)
            .enabled_features(&features)
            .enabled_extension_names(&extensions);

        let device = unsafe { instance.create_device(*physical_device, &device_create_info, None)? };
        Ok(device)
    }

    /// Assigns an increasing score based on the available features, or 0 when geometry shaders are not supported.
    fn device_suitability_score(physical_device: &vk::PhysicalDevice, instance: &Instance) -> i32 {
        let queue_family = QueueFamilyIndices::new(physical_device, instance);
        if queue_family.is_err() { return 0; }

        let properties = unsafe { instance.get_physical_device_properties(*physical_device) };
        let features = unsafe { instance.get_physical_device_features(*physical_device) };
        let mut score: i32 = 0;
        if features.geometry_shader == vk::FALSE { score += 2000; }
        if properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU { score += 1000; }

        score += properties.limits.max_image_dimension2_d as i32;
        return score;
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

        let layers_names_raw: Vec<*const c_char> = debug::VALIDATION_LAYERS
            .iter()
            .map(|raw_name| raw_name.as_ptr())
            .collect();

        let mut debug_info = debug::create_debug_info();
        if debug::VALIDATION_ENABLED {
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
        unsafe { self.logical_device.destroy_device(None) };
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
