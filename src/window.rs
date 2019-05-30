use crate::consts::APP_TITLE;

#[derive(Debug)]
pub struct WindowState {
    pub events_loop: winit::EventsLoop,
    pub wb: Option<winit::WindowBuilder>,
}

// TODO: config
pub const DEFAULT_WIDTH: u32 = 1024;
pub const DEFAULT_HEIGHT: u32 = 768;

impl WindowState {
    pub fn new() -> WindowState {
        let events_loop = winit::EventsLoop::new();

        let wb = winit::WindowBuilder::new()
            .with_dimensions(winit::dpi::LogicalSize::new(
                DEFAULT_WIDTH as _,
                DEFAULT_HEIGHT as _))
            .with_title(APP_TITLE);

        WindowState {
            events_loop,
            wb: Some(wb)
        }
    }
}
