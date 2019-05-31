extern crate winit;
extern crate gfx_hal;
extern crate env_logger;
extern crate log;
extern crate nalgebra_glm as glm;

mod consts;
mod window;
mod rendering;

use env_logger::Env;
use window::WindowState;

fn main() {
    env_logger::from_env(Env::default().default_filter_or("trace")).init();

    let mut window = WindowState::new();
    let (backend, _instance) = rendering::create_backend(&mut window);
    let mut renderer_state = unsafe {
        rendering::RendererState::new(backend, window)
    };
    renderer_state.mainloop();
}
