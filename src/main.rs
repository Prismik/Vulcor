mod swapchain;
mod synchronous;
mod core;
mod pipeline;
mod math;
mod cmd;

use anyhow::{anyhow, Result};
use ash::{ext::debug_utils, vk::{self, Handle}, Device};
use std::{error::Error, ffi::{CString}};
use log::{info};
use winit::{
    application::ApplicationHandler, event::WindowEvent, 
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, 
    window::{Window, WindowId}
};

use crate::{
    cmd::command_pool::CmdPool, 
    core::{context::VulkanContext, graphics::Devices, physical_device::QueueFamilyIndices}, 
    pipeline::{render_pipeline::{RenderPipeline}, traits::VulkanPipeline}, 
    swapchain::{SwapchainConfig, SwapchainData}
};


struct App {
    name: String,
    vulcor: Option<Vulcor>,
    minimized: bool
}

impl App {
    fn new() -> App {
        Self { name: "Vulcor".to_string(), vulcor: None, minimized: false }
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
    vertex_buffer: vk::Buffer,
    vertex_buffer_mem: vk::DeviceMemory,
    command_pool: CmdPool,
    command_buffers: Vec<vk::CommandBuffer>,
    sync: synchronous::RenderSync,
    run: bool,
    resized: bool
}

impl Vulcor {
    fn new(window: Window) -> Result<Self, Box<dyn Error>> {
        info!("Creating application");
        let title = "Vulcor";
        let context = VulkanContext::new(CString::new(title)?.as_c_str(), &window)?;
        let messenger = core::debug::setup_debug_messenger(&context);
        let devices = Devices::new(&context)?;
        let queue_family = QueueFamilyIndices::new(&context, &devices.physical.instance)?;
        let graphics_queue = unsafe { devices.logical.instance.get_device_queue(queue_family.graphics, 0) };
        let presentation_queue = unsafe { devices.logical.instance.get_device_queue(queue_family.presentation, 0) };
        let swapchain = swapchain::SwapchainData::new(&context, &devices.logical.instance, &devices.physical.instance, &window)?;
        let render_pass = Self::create_render_pass(&devices.logical.instance, &swapchain.config)?;
        let pipeline = RenderPipeline::new(&devices.logical.instance, &swapchain.config, &render_pass)?;
        let framebuffers = Self::create_framebuffers(&devices, &swapchain, &render_pass)?;
        let command_pool = CmdPool::new(&devices.logical, &queue_family)?;
        let (vertex_buffer, vertex_buffer_mem) = unsafe { devices.create_vertex_buffer(&context)? };
        let command_buffers = unsafe { command_pool.create_buffers(framebuffers.len() as u32, &devices.logical, &render_pass, &pipeline.instance(), &framebuffers, &vertex_buffer, &swapchain)? };
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
            vertex_buffer,
            vertex_buffer_mem,
            command_pool,
            command_buffers,
            sync,
            run: true,
            resized: false
        })
    }

    fn recreate_swapchain(&mut self) -> Result<()> {
        unsafe { self.devices.logical.instance.device_wait_idle()?; }
        self.destroy_swapchain();
        self.swapchain = swapchain::SwapchainData::new(&self.context, &self.devices.logical.instance, &self.devices.physical.instance, &self.window)?;
        self.render_pass = Self::create_render_pass(&self.devices.logical.instance, &self.swapchain.config)?;
        self.pipeline = RenderPipeline::new(&self.devices.logical.instance, &self.swapchain.config, &self.render_pass)?;
        self.framebuffers = Self::create_framebuffers(&self.devices, &self.swapchain, &self.render_pass)?;
        self.command_buffers = unsafe { self.command_pool.create_buffers(self.framebuffers.len() as u32, &self.devices.logical, &self.render_pass, &self.pipeline.instance(), &self.framebuffers, &self.vertex_buffer, &self.swapchain)? };
        self.sync = synchronous::RenderSync::new(&self.devices, &self.swapchain)?;
        Ok(())
    }

    fn create_framebuffers(devices: &Devices, swapchain: &SwapchainData, render_pass: &vk::RenderPass) -> Result<Vec<vk::Framebuffer>> {
        let framebuffers = swapchain.image_views.iter()
            .map(|img| {
                let attachments = &[*img];
                let create_info = vk::FramebufferCreateInfo::default()
                    .render_pass(*render_pass)
                    .attachments(attachments)
                    .width(swapchain.config.extent.width)
                    .height(swapchain.config.extent.height)
                    .layers(1);
                unsafe { devices.logical.instance.create_framebuffer(&create_info, None).unwrap() }
            })
            .collect::<Vec<_>>();

        Ok(framebuffers)
    }

    fn create_render_pass(logical_device: &Device, swapchain: &SwapchainConfig) -> Result<vk::RenderPass> {
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

    fn render(&mut self) -> Result<()> {
        unsafe { self.devices.logical.instance.wait_for_fences(&[self.sync.get_in_flight_fence()], true, u64::MAX)? };        
        let result = unsafe { self.swapchain.loader.acquire_next_image(
                self.swapchain.khr, 
                u64::MAX, 
                self.sync.get_image_available(), 
                vk::Fence::null()
            )
        };

        let image_index = match result {
            Ok((image_index, _)) => image_index as usize,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => return self.recreate_swapchain(),
            Err(e) => return Err(anyhow!(e)),
        };
        
        // TODO Possibly encapsulate in sync object 
        let in_flight = self.sync.images_in_flight[image_index as usize];
        if !in_flight.is_null() {
            unsafe { self.devices.logical.instance.wait_for_fences(&[in_flight], true, u64::MAX)? };
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
        let _ = unsafe { self.devices.logical.instance.queue_submit(self.graphics_queue, &[submit_info], self.sync.get_in_flight_fence()) };
        let swapchains = &[self.swapchain.khr];
        let image_indices = &[image_index as u32];
        let present_info = vk::PresentInfoKHR::default()
            .wait_semaphores(signal_semaphores)
            .swapchains(swapchains)
            .image_indices(image_indices);
        let result = unsafe { self.swapchain.loader.queue_present(self.presentation_queue, &present_info) };
        if self.resized {
            self.resized = false;
            self.recreate_swapchain()?;
        } else {
            match result {
                Ok(false) => self.recreate_swapchain()?,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.recreate_swapchain()?,
                Err(e) => return Err(anyhow!(e)),
                _ => {}
            }
        }
        self.sync.increment_frame();
        Ok(())
    }

    fn cleanup(&mut self) {
        println!("Cleaning up resources...");
        let _ = unsafe { self.devices.logical.instance.device_wait_idle() };
        unsafe {
            self.sync.cleanup(&self.devices);
            self.destroy_swapchain();
            self.devices.logical.instance.destroy_buffer(self.vertex_buffer, None);
            self.devices.logical.instance.free_memory(self.vertex_buffer_mem, None);
            self.devices.logical.instance.destroy_command_pool(self.command_pool.instance, None);
            self.devices.logical.instance.destroy_device(None);
            if let Some((report, callback)) = self.messenger.as_ref().take() {
                report.destroy_debug_utils_messenger(*callback, None);
            }
            self.context.cleanup();
        }
    }

    fn destroy_swapchain(&mut self) {
        unsafe {
            self.framebuffers.iter()
                .for_each(|f| self.devices.logical.instance.destroy_framebuffer(*f, None));
            self.devices.logical.instance.free_command_buffers(self.command_pool.instance, &self.command_buffers);
            self.pipeline.cleanup(&self.devices.logical.instance);
            self.devices.logical.instance.destroy_render_pass(self.render_pass, None);
            self.swapchain.cleanup(&self.devices);
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
                    WindowEvent::Resized(size) => { 
                        self.minimized = size.width == 0 || size.height == 0;
                        if !self.minimized {
                            instance.resized = true; 
                        }
                    },
                    _ => (),
                }
            }
            _ => ()
        }

    }
}

fn main() -> Result<()> {
    let mut app = App::new();
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut app)?;

    Ok(())
}
