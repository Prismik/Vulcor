mod debug;
mod swapchain;
mod devices;
mod synchronous;

use ash::{ext::debug_utils, khr::surface, vk::{self, Handle}, Device, Entry, Instance};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::{
    collections::{HashSet}, env, error::Error, ffi::{c_char, CStr, CString}, path::PathBuf, result::Result
};
use log::{info};
use winit::{
    application::ApplicationHandler, event::WindowEvent, 
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, window::{Window, WindowAttributes, WindowId}
};
use crate::{devices::{Devices, PhysicalDeviceError}, swapchain::{SwapchainConfig, SwapchainData}};

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
    devices: Devices,
    graphics_queue: vk::Queue,
    presentation_queue: vk::Queue,
    surface: vk::SurfaceKHR,
    surface_loader: surface::Instance,
    swapchain: SwapchainData,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    framebuffers: Vec<vk::Framebuffer>,
    command_pool: vk::CommandPool,
    command_buffers: Vec<vk::CommandBuffer>,
    sync: synchronous::RenderSync,
    run: bool
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
        let devices = devices::Devices::new(&entry, &instance, &surface, &surface_loader)?;
        let queue_family = QueueFamilyIndices::new(&devices.physical, &entry, &instance, &surface, &surface_loader)?;
        let graphics_queue = unsafe { devices.logical.get_device_queue(queue_family.graphics, 0) };
        let presentation_queue = unsafe { devices.logical.get_device_queue(queue_family.presentation, 0) };
        let swapchain = swapchain::SwapchainData::new(&entry, &instance, &devices.logical, &devices.physical, &surface, &window, &surface_loader)?;
        let render_pass = Self::create_render_pass(&devices.logical, &swapchain.config)?;
        let (pipeline, pipeline_layout) = Self::create_pipeline(&devices.logical, &swapchain.config, &render_pass)?;
        let framebuffers = Self::create_framebuffers(&devices, &swapchain, &render_pass)?;
        let command_pool = Self::create_command_pool(&devices, &queue_family)?;
        let command_buffers = Self::create_command_buffers(framebuffers.len() as u32, &devices, &command_pool, &render_pass, &pipeline, &framebuffers, &swapchain)?;
        let sync = synchronous::RenderSync::new(&devices, &swapchain)?;
        Ok(Self{
            name: title.to_string(),  
            window,
            entry,
            instance,
            messenger,
            devices,
            graphics_queue,
            presentation_queue,
            surface,
            surface_loader,
            swapchain,
            render_pass,
            pipeline_layout,
            pipeline,
            framebuffers,
            command_pool,
            command_buffers,
            sync,
            run: true
        })
    }

    fn create_command_buffers(count: u32, devices: &Devices, command_pool: &vk::CommandPool, render_pass: &vk::RenderPass, pipeline: &vk::Pipeline, framebuffers: &Vec<vk::Framebuffer>, swapchain: &SwapchainData) -> Result<Vec<vk::CommandBuffer>, Box<dyn Error>> {
        let allocate_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(*command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(count);

        let command_buffers = unsafe { devices.logical.allocate_command_buffers(&allocate_info)? };
        command_buffers.iter()
            .zip(framebuffers.iter())
            .for_each(|(command_buffer, framebuffer)| {
                let inheritance = vk::CommandBufferInheritanceInfo::default();
                let info = vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::empty())
                    .inheritance_info(&inheritance);
                let _ = unsafe { devices.logical.begin_command_buffer(*command_buffer, &info) };
                let render_area = vk::Rect2D::default()
                    .offset(vk::Offset2D::default())
                    .extent(swapchain.config.extent);
                let clear_color_value = vk::ClearValue {
                    color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 1.0] }
                };
                let clear_values = &[clear_color_value];
                let begin_info = vk::RenderPassBeginInfo::default()
                    .render_pass(*render_pass)
                    .framebuffer(*framebuffer)
                    .render_area(render_area)
                    .clear_values(clear_values);

                // Setup commands
                unsafe { devices.logical.cmd_begin_render_pass(*command_buffer, &begin_info, vk::SubpassContents::INLINE) };
                unsafe { devices.logical.cmd_bind_pipeline(*command_buffer, vk::PipelineBindPoint::GRAPHICS, *pipeline) };
                unsafe { devices.logical.cmd_draw(*command_buffer, 3, 1, 0, 0) };
                unsafe { devices.logical.cmd_end_render_pass(*command_buffer) };
                let _ = unsafe { devices.logical.end_command_buffer(*command_buffer) };
            });
        Ok(command_buffers)
    }

    fn create_command_pool(devices: &Devices, queue_family: &QueueFamilyIndices) -> Result<vk::CommandPool, Box<dyn Error>> {
        let create_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::empty())
            .queue_family_index(queue_family.graphics);

        let command_pool = unsafe { devices.logical.create_command_pool(&create_info, None)? };
        Ok(command_pool)
    }

    fn create_framebuffers(devices: &Devices, swapchain: &SwapchainData, render_pass: &vk::RenderPass) -> Result<Vec<vk::Framebuffer>, Box<dyn Error>> {
        let framebuffers = swapchain.image_views.iter()
            .map(|img| {
                let attachments = &[*img];
                let create_info = vk::FramebufferCreateInfo::default()
                    .render_pass(*render_pass)
                    .attachments(attachments)
                    .width(swapchain.config.extent.width)
                    .height(swapchain.config.extent.height)
                    .layers(1);
                unsafe { devices.logical.create_framebuffer(&create_info, None).unwrap() }
            })
            .collect::<Vec<_>>();

        Ok(framebuffers)
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
        let dependency = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);
        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(color_attachments);
        let attachments = &[color_attachment];
        let supbasses = &[subpass];
        let dependencies = &[dependency];
        let create_info  = vk::RenderPassCreateInfo::default()
            .attachments(attachments)
            .subpasses(supbasses)
            .dependencies(dependencies);
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

    fn render(&mut self) -> Result<(), Box<dyn Error>> {
        unsafe { self.devices.logical.wait_for_fences(&[self.sync.get_in_flight_fence()], true, u64::MAX)? };
        let image_index = unsafe { 
            self.swapchain.loader.acquire_next_image(
                self.swapchain.khr, 
                u64::MAX, 
                self.sync.get_image_available(), 
                vk::Fence::null()
            ).unwrap_or((0, false)).0 as usize 
        };
        
        // TODO Possibly encapsulate in sync object 
        let in_flight = self.sync.images_in_flight[image_index as usize];
        if !in_flight.is_null() {
            unsafe { self.devices.logical.wait_for_fences(&[in_flight], true, u64::MAX)? };
        }
        self.sync.update_image_in_flight(image_index);

        let wait_semaphores = &[self.sync.get_image_available()];
        let wait_stages = &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
        let command_buffers = &[self.command_buffers[image_index]];
        let signal_semaphores = &[self.sync.get_render_completed()];
        let submit_info = vk::SubmitInfo::default()
            .wait_semaphores(wait_semaphores)
            .wait_dst_stage_mask(wait_stages)
            .command_buffers(command_buffers)
            .signal_semaphores(signal_semaphores);

        self.sync.reset_fences(&self.devices)?;
        let _ = unsafe { self.devices.logical.queue_submit(self.graphics_queue, &[submit_info], self.sync.get_in_flight_fence()) };
        let swapchains = &[self.swapchain.khr];
        let image_indices = &[image_index as u32];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(signal_semaphores)
            .swapchains(swapchains)
            .image_indices(image_indices);
        unsafe { self.swapchain.loader.queue_present(self.presentation_queue, &present_info)? };
        self.sync.increment_frame();
        Ok(())
    }

    fn cleanup(&self) {
        println!("Cleaning up resources...");
        let _ = unsafe { self.devices.logical.device_wait_idle() };
        unsafe {
            self.sync.cleanup(&self.devices);
            self.devices.logical.destroy_command_pool(self.command_pool, None);
            self.framebuffers.iter()
                .for_each(|f| self.devices.logical.destroy_framebuffer(*f, None));
            self.devices.logical.destroy_pipeline(self.pipeline, None);
            self.devices.logical.destroy_pipeline_layout(self.pipeline_layout, None);
            self.devices.logical.destroy_render_pass(self.render_pass, None);
            self.swapchain.image_views.iter()
                .for_each(|v| self.devices.logical.destroy_image_view(*v, None));
            self.swapchain.loader.destroy_swapchain(self.swapchain.khr, None);
            self.devices.logical.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let app = self.vulcor.as_mut().unwrap();
        if app.run {
            app.render();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match self.vulcor.as_mut() {
            Some(instance) => {
                match event {
                    WindowEvent::RedrawRequested => {
                        let result = instance.render();
                        if result.is_err() { log::error!("Error occured on a render pass"); }
                    },
                    WindowEvent::CloseRequested => {
                        instance.run = false;
                        instance.cleanup();
                        event_loop.exit();
                    },
                    _ => (),
                }
            }
            _ => ()
        }

    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut app = App::new();
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut app)?;

    Ok(())
}
