mod debug;

use ash::{ext::debug_utils, khr::{surface, swapchain}, vk::{self, ImageView, SwapchainKHR}, Device, Entry, Instance};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    collections::{BTreeMap, HashSet}, error::Error, ffi::{c_char, CStr, CString}, fmt::{self, Display, Formatter}, result::Result
};
use log::{debug, info, error, warn, trace};
use winit::{
    application::ApplicationHandler, event::{Event, WindowEvent}, 
    event_loop::{ActiveEventLoop, EventLoop}, window::{Window, WindowId}
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
    presentation: u32
}

impl QueueFamilyIndices {
    fn new(physical_device: &vk::PhysicalDevice, entry: &Entry, instance: &Instance, surface: &vk::SurfaceKHR, loader: &surface::Instance) -> Result<Self, Box<dyn Error>> {
        let properties = unsafe { instance.get_physical_device_queue_family_properties(*physical_device) };

        // TODO Unify both graphics and presentation queues
        let graphics = properties
            .iter()
            .position(|p| p.queue_flags.contains(vk::QueueFlags::GRAPHICS))
            .map(|i| i as u32);

        let mut presentation = None;
        for (index, _) in properties.iter().enumerate() {
            let supported = unsafe { loader.get_physical_device_surface_support(*physical_device, index as u32, *surface)? };
            if supported {
                presentation = Some(index as u32);
                break;
            }
        }
        
        if let (Some(graphics), Some(presentation)) = (graphics, presentation) {
            Ok(Self { graphics, presentation })
        } else {
            Err(Box::new(PhysicalDeviceError::NoSuitableQueueFamily))
        }
    }

    fn unique_values(&self) -> HashSet<u32> {
        return HashSet::from([self.graphics, self.presentation]);
    }
}

#[derive(Clone, Debug)]
struct SwapchainSupport {
    capabilities: vk::SurfaceCapabilitiesKHR,
    formats: Vec<vk::SurfaceFormatKHR>,
    present_modes: Vec<vk::PresentModeKHR>,
}

impl SwapchainSupport {
    fn new(entry: &Entry, instance: &Instance, physical_device: &vk::PhysicalDevice, surface: &vk::SurfaceKHR) -> Result<Self, Box<dyn Error>> {
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
    graphics_queue: vk::Queue,
    presentation_queue: vk::Queue,
    surface: vk::SurfaceKHR,
    surface_loader: surface::Instance,
    swapchain: SwapchainKHR,
    swapchain_loader: swapchain::Device,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
}

impl Vulcor {
    fn new(window: Window) -> Result<Self, Box<dyn Error>> {
        info!("Creating application");
        let title = "Vulcor";
        let entry = Entry::linked();
        let instance = Self::create_instance(CString::new(title)?.as_c_str(), &entry, &window)?;
        let messenger = debug::setup_debug_messenger(&entry, &instance);
        let surface = unsafe { ash_window::create_surface(
            &entry, 
            &instance, 
            window.display_handle()?.as_raw(), 
            window.window_handle()?.as_raw(), 
            None
        )? };
        let surface_loader = surface::Instance::new(&entry, &instance);
        let physical_device = Self::select_physical_device(&entry, &instance, &surface, &surface_loader)?;
        let logical_device = Self::create_logical_device(&physical_device, &entry, &instance, &surface, &surface_loader)?;
        let queue_family = QueueFamilyIndices::new(&physical_device, &entry, &instance, &surface, &surface_loader)?;
        let graphics_queue = unsafe { logical_device.get_device_queue(queue_family.graphics, 0) };
        let presentation_queue = unsafe { logical_device.get_device_queue(queue_family.presentation, 0) };
        let swapchain_loader = swapchain::Device::new(&instance, &logical_device);
        let (swapchain, format) = Self::create_swapchain(&entry, &instance, &physical_device, &surface, &window, &surface_loader, &swapchain_loader)?;
        let swapchain_images = unsafe { swapchain_loader.get_swapchain_images(swapchain)? };
        let swapchain_image_views = Self::create_image_views(&logical_device, &swapchain_images, &format)?;
        Ok(Self{
            name: title.to_string(),  
            window,
            entry,
            instance,
            messenger,
            physical_device,
            logical_device,
            graphics_queue,
            presentation_queue,
            surface,
            surface_loader,
            swapchain,
            swapchain_loader,
            swapchain_images,
            swapchain_image_views,
        })
    }

    fn select_physical_device(entry: &Entry, instance: &Instance, surface: &vk::SurfaceKHR, surface_loader: &surface::Instance) -> Result<vk::PhysicalDevice, Box<dyn Error>> {
        let devices = unsafe { instance.enumerate_physical_devices()? };
        let mut candidates: BTreeMap<i32, vk::PhysicalDevice> = BTreeMap::new();

        for physical_device in devices {
            let score = Self::device_suitability_score(&physical_device, entry, instance, surface, surface_loader);
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
    fn device_suitability_score(physical_device: &vk::PhysicalDevice, entry: &Entry, instance: &Instance, surface: &vk::SurfaceKHR, surface_loader: &surface::Instance) -> i32 {
        let queue_family = QueueFamilyIndices::new(physical_device, entry, instance, surface, surface_loader);
        if queue_family.is_err() { return 0; }
        if !Self::device_supports_extensions(physical_device, instance) { return 0; }
    
        let swapchain_support = SwapchainSupport::new(entry, instance, physical_device, surface);
        if swapchain_support.is_err() { return 0; }
        if let Ok(support) = swapchain_support {
            if support.formats.is_empty() || support.present_modes.is_empty() { return 0; }
        }
    
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
        let mut extensions = vec![swapchain::NAME];
        // Required by Vulkan SDK on macOS since 1.3.216.
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            extensions.push(ash::khr::portability_subset::NAME);
        }
        return extensions
    }

    fn create_swapchain(entry: &Entry, instance: &Instance, physical_device: &vk::PhysicalDevice, surface: &vk::SurfaceKHR, window: &Window, surface_loader: &surface::Instance, swapchain_loader: &swapchain::Device) -> Result<(vk::SwapchainKHR, vk::SurfaceFormatKHR), Box<dyn Error>> {
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

        let swapchain = unsafe { swapchain_loader.create_swapchain(&swapchain_create_info, None)? };
        Ok((swapchain, format))
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

    fn create_pipeline(device: &Device) {

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
        //unsafe { self.logical_device.device_wait_idle() };
        unsafe { 
            self.swapchain_image_views
                .iter()
                .for_each(|v| self.logical_device.destroy_image_view(*v, None));
            self.logical_device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.swapchain_loader.destroy_swapchain(self.swapchain, None);
            if let Some((report, callback)) = self.messenger.as_ref().take() {
                report.destroy_debug_utils_messenger(*callback, None);
            }
            self.instance.destroy_instance(None);
        }
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
