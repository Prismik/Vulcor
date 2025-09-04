use std::{error::Error, result::Result};
use log::{info, error};
use winit::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowAttributes, WindowId},
    application::{ApplicationHandler}
};

struct Vulcor {
    window: Option<Window>,
}

impl Vulcor {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        info!("Creating application");
        Ok(Self{window:None})
    }

    fn init(&self) {

    }

    fn render(&self) {
        println!("render()");
    }

    fn cleanup(&self) {

    }
}

impl ApplicationHandler for Vulcor {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes().with_title("Vulcor");
        self.window = Some(event_loop.create_window(window_attributes).unwrap());
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        println!("{event:?}");
        match event {
            WindowEvent::RedrawRequested => self.render(),
            WindowEvent::CloseRequested => {
                self.cleanup();
                event_loop.exit()
            },
            _ => (),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut app = Vulcor::new()?;
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;

    Ok(())
}
