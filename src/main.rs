mod swapchain;
mod synchronous;
mod core;
mod pipeline;

use ash::{ext::debug_utils, vk::{self, Handle}, Device};
use std::{error::Error, ffi::{CString}, result::Result};
use log::{info};
use winit::{
    application::ApplicationHandler, event::WindowEvent, 
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, 
    window::{Window, WindowId}
};
use crate::{
    core::{context::VulkanContext, logical_device::Devices, physical_device::QueueFamilyIndices}, 
    pipeline::{render_pipeline::RenderPipeline, traits::VulkanPipeline}, 
    swapchain::{SwapchainConfig, SwapchainData}
};

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
    context: VulkanContext,
    messenger: Option<(debug_utils::Instance, vk::DebugUtilsMessengerEXT)>,
    devices: Devices,
    graphics_queue: vk::Queue,
    presentation_queue: vk::Queue,
    swapchain: SwapchainData,
    render_pass: vk::RenderPass,
    pipeline: RenderPipeline,
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
        let context = VulkanContext::new(CString::new(title)?.as_c_str(), &window)?;
        let messenger = core::debug::setup_debug_messenger(&context);
        let devices = Devices::new(&context)?;
        let queue_family = QueueFamilyIndices::new(&context, &devices.physical)?;
        let graphics_queue = unsafe { devices.logical.get_device_queue(queue_family.graphics, 0) };
        let presentation_queue = unsafe { devices.logical.get_device_queue(queue_family.presentation, 0) };
        let swapchain = swapchain::SwapchainData::new(&context, &devices.logical, &devices.physical, &window)?;
        let render_pass = Self::create_render_pass(&devices.logical, &swapchain.config)?;
        let pipeline = RenderPipeline::new(&devices.logical, &swapchain.config, &render_pass)?;
        let framebuffers = Self::create_framebuffers(&devices, &swapchain, &render_pass)?;
        let command_pool = Self::create_command_pool(&devices, &queue_family)?;
        let command_buffers = Self::create_command_buffers(framebuffers.len() as u32, &devices, &command_pool, &render_pass, &pipeline.instance(), &framebuffers, &swapchain)?;
        let sync = synchronous::RenderSync::new(&devices, &swapchain)?;
        Ok(Self{
            name: title.to_string(),  
            window,
            context,
            messenger,
            devices,
            graphics_queue,
            presentation_queue,
            swapchain,
            render_pass,
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
            self.pipeline.cleanup(&self.devices.logical);
            self.devices.logical.destroy_render_pass(self.render_pass, None);
            self.swapchain.cleanup(&self.devices);
            self.devices.logical.destroy_device(None);
            if let Some((report, callback)) = self.messenger.as_ref().take() {
                report.destroy_debug_utils_messenger(*callback, None);
            }
            self.context.cleanup();
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

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        let app = self.vulcor.as_mut().unwrap();
        if app.run {
            let _ = app.render();
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
