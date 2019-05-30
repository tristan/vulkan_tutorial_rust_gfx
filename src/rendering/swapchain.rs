use std::cell::RefCell;
use std::rc::Rc;
use log::debug;
use gfx_hal::{Backend, Device, Surface, SwapchainConfig};
use gfx_hal::image::Extent;
use gfx_hal::format::{ChannelType, Format};
use gfx_hal::window::Extent2D;
use super::device::DeviceState;
use super::BackendState;

use crate::window::{DEFAULT_WIDTH, DEFAULT_HEIGHT};

const DEFAULT_EXTENT: Extent2D = Extent2D {
    width: DEFAULT_WIDTH,
    height: DEFAULT_HEIGHT
};

pub(super) struct SwapchainState<B: Backend> {
    pub(super) swapchain: Option<B::Swapchain>,
    pub(super) backbuffer: Option<Vec<B::Image>>,
    pub(super) extent: Extent,
    pub(super) format: Format,
    device: Rc<RefCell<DeviceState<B>>>,
}

impl<B: Backend> SwapchainState<B> {
    pub(super) unsafe fn new(backend: &mut BackendState<B>, device: Rc<RefCell<DeviceState<B>>>) -> Self {
        let (caps, formats, _present_modes) = backend
            .surface
            .compatibility(&device.borrow().physical_device);
        debug!("formats: {:?}", formats);
        let format = formats.map_or(Format::Rgba8Srgb, |formats| {
            formats
                .iter()
                .find(|format| format.base_format().1 == ChannelType::Srgb)
                .map(|format| *format)
                .unwrap_or(formats[0])
        });
        debug!("Surface format: {:?}", format);
        let swap_config = SwapchainConfig::from_caps(&caps, format, DEFAULT_EXTENT);
        debug!("Swapchain Config: {:?}", swap_config);
        let extent = swap_config.extent.to_extent();
        debug!("Extent: {:?}", extent);
        let (swapchain, backbuffer) = device
            .borrow()
            .device
            .create_swapchain(&mut backend.surface, swap_config, None)
            .expect("Can't create swapchain");

        SwapchainState {
            swapchain: Some(swapchain),
            backbuffer: Some(backbuffer),
            device,
            extent,
            format
        }
    }
}

impl<B: Backend> Drop for SwapchainState<B> {
    fn drop(&mut self) {
        debug!("~~DROP SwapchainState");
        unsafe {
            self.device
                .borrow()
                .device
                .destroy_swapchain(self.swapchain.take().unwrap());
        }
    }
}
