mod debug;
mod swapchain;

use ash::{ext::debug_utils, khr::{surface}, vk::{self, ImageView, SwapchainKHR}, Device, Entry, Instance};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    collections::{BTreeMap, HashSet}, env, error::Error, ffi::{c_char, CStr, CString}, fmt::{self, Display, Formatter}, path::PathBuf, result::Result
};
use log::{debug, info, error, warn, trace};
use winit::{
    application::ApplicationHandler, event::{Event, WindowEvent}, 
    event_loop::{ActiveEventLoop, EventLoop}, window::{Window, WindowId}
};

use crate::swapchain::{SwapchainConfig, SwapchainData, SwapchainSupport};

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
    swapchain: swapchain::SwapchainData,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline
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
        let swapchain = swapchain::SwapchainData::new(&entry, &instance, &logical_device, &physical_device, &surface, &window, &surface_loader)?;
        let render_pass = Self::create_render_pass(&logical_device, &swapchain.config)?;
        let (pipeline, pipeline_layout) = Self::create_pipeline(&logical_device, &swapchain.config, &render_pass)?;
        
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
            render_pass,
            pipeline_layout,
            pipeline
        })
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

    fn create_render_pass(logical_device: &Device, swapchain: &SwapchainConfig) -> Result<vk::RenderPass, Box<dyn Error>> {
        let color_attachment = vk::AttachmentDescription::default()
            .format(swapchain.format.format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);
        let color_attachment_ref = vk::AttachmentReference::default()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);
        let color_attachments = &[color_attachment_ref];
        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(color_attachments);
        let attachments = &[color_attachment];
        let supbasses = &[subpass];
        let create_info  = vk::RenderPassCreateInfo::default()
            .attachments(attachments)
            .subpasses(supbasses);
        let render_pass = unsafe { logical_device.create_render_pass(&create_info, None)? };
        Ok(render_pass)
    }

    fn create_pipeline(logical_device: &Device, config: &SwapchainConfig, render_pass: &vk::RenderPass) -> Result<(vk::Pipeline, vk::PipelineLayout), Box<dyn Error>> {
        let vert = Self::read_shader_file("shaders/vert.spv")?;
        let frag = Self::read_shader_file("shaders/frag.spv")?;
        let vert_shader_module = Self::create_shader_module(logical_device, &vert)?;
        let frag_shader_module = Self::create_shader_module(logical_device, &frag)?;

        let main = CString::new("main")?;
        let vert_stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vert_shader_module)
            .name(main.as_c_str());
        let frag_stage = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(frag_shader_module)
            .name(main.as_c_str());

        let vert_input_state = vk::PipelineVertexInputStateCreateInfo::default();
        let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        let viewport = vk::Viewport::default()
            .x(0.0)
            .y(0.0)
            .width(config.extent.width as f32)
            .height(config.extent.height as f32)
            .min_depth(0.0)
            .max_depth(1.0);
        let scissor = vk::Rect2D::default()
            .offset(vk::Offset2D { x: 0, y: 0 })
            .extent(config.extent);
        let viewports = &[viewport];
        let scissors = &[scissor];
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewports(viewports)
            .scissors(scissors);
        let rasterization_state = vk::PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::CLOCKWISE)
            .depth_bias_enable(false);

        let multisample_state = vk::PipelineMultisampleStateCreateInfo::default()
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);

        // Blending can be changed here
        let color_blend_attachment_state = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ZERO)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD);
        let attachments = &[color_blend_attachment_state];
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op_enable(false)
            .logic_op(vk::LogicOp::COPY)
            .attachments(attachments)
            .blend_constants([0.0, 0.0, 0.0, 0.0]);
        
        let layout_info = vk::PipelineLayoutCreateInfo::default();
        let pipeline_layout = unsafe { logical_device.create_pipeline_layout(&layout_info, None)? };

        let stages = &[vert_stage, frag_stage];
        let graphics_pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(stages)
            .vertex_input_state(&vert_input_state)
            .input_assembly_state(&input_assembly_state)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterization_state)
            .multisample_state(&multisample_state)
            .color_blend_state(&color_blend_state)
            .layout(pipeline_layout)
            .render_pass(*render_pass)
            .subpass(0)
            .base_pipeline_handle(vk::Pipeline::null())
            .base_pipeline_index(-1);
        let pipeline = unsafe { 
            logical_device.create_graphics_pipelines(
                vk::PipelineCache::null(), 
                &[graphics_pipeline_info], 
                None
            ).as_ref().unwrap()[0]
        };
        unsafe { logical_device.destroy_shader_module(vert_shader_module, None) };
        unsafe { logical_device.destroy_shader_module(frag_shader_module, None) };
        Ok((pipeline, pipeline_layout))
    }

    fn read_shader_file<P: AsRef<std::path::Path>>(path: P) -> Result<Vec<u32>, Box<dyn Error>> {
        let current_dir = env::current_dir()?;
        let mut target = PathBuf::from(current_dir);
        target.push(path);
        log::debug!("Loading shader => {}", target.to_string_lossy());
        let mut file = std::fs::File::open(target)?;
        Ok(ash::util::read_spv(&mut file)?)
    }

    fn create_shader_module(logical_device: &Device, code: &[u32]) -> Result<vk::ShaderModule, Box<dyn Error>> {
        let create_info = vk::ShaderModuleCreateInfo::default().code(code);
        let module = unsafe { logical_device.create_shader_module(&create_info, None)? };
        Ok(module)
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
            self.logical_device.destroy_pipeline(self.pipeline, None);
            self.logical_device.destroy_render_pass(self.render_pass, None);
            self.logical_device.destroy_pipeline_layout(self.pipeline_layout, None);
            self.swapchain.image_views
                .iter()
                .for_each(|v| self.logical_device.destroy_image_view(*v, None));
            self.logical_device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            self.swapchain.loader.destroy_swapchain(self.swapchain.swapchain, None);
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
                    Err(error) => panic!("FATAL ERROR ENCOUNTERED => {}", error)
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
