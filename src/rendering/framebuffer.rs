use std::cell::RefCell;
use std::rc::Rc;
use gfx_hal::{Backend, Device};
use gfx_hal::format;
use gfx_hal::image::{Extent, ViewKind};
use super::device::DeviceState;
use super::render_pass::RenderPassState;
use super::swapchain::SwapchainState;

use super::constants::COLOR_RANGE;

pub(super) struct FramebufferState<B: Backend> {
    pub(super) frame_images: Option<Vec<(B::Image, B::ImageView)>>,
    pub(super) framebuffers: Option<Vec<B::Framebuffer>>,
    device: Rc<RefCell<DeviceState<B>>>
}

impl<B: Backend> FramebufferState<B> {
    pub(super) unsafe fn new(
        device: Rc<RefCell<DeviceState<B>>>,
        render_pass: &RenderPassState<B>,
        swapchain: &mut SwapchainState<B>,
    ) -> Self {
        let (frame_images, framebuffers) = {
            let extent = Extent {
                width: swapchain.extent.width as _,
                height: swapchain.extent.height as _,
                depth: 1,
            };

            let pairs = swapchain.backbuffer.take().unwrap().into_iter()
                .map(|image| {
                    let rtv = device
                        .borrow()
                        .device
                        .create_image_view(
                            &image,
                            ViewKind::D2,
                            swapchain.format,
                            format::Swizzle::NO,
                            COLOR_RANGE.clone()
                        )
                        .unwrap();
                    (image, rtv)
                })
                .collect::<Vec<_>>();

            let fbos = pairs
                .iter()
                .map(|&(_, ref rtv)| {
                    device
                        .borrow()
                        .device
                        .create_framebuffer(
                            render_pass.render_pass.as_ref().unwrap(),
                            Some(rtv),
                            extent,
                        )
                        .unwrap()
                })
                .collect();

            (pairs, fbos)
        };

        FramebufferState {
            frame_images: Some(frame_images),
            framebuffers: Some(framebuffers),
            device
        }
    }
}

impl<B: Backend> Drop for FramebufferState<B> {
    fn drop(&mut self) {
        let device = &self.device.borrow().device;
        unsafe {
            for framebuffer in self.framebuffers.take().unwrap() {
                device.destroy_framebuffer(framebuffer);
            }

            for (_, rtv) in self.frame_images.take().unwrap() {
                device.destroy_image_view(rtv);
            }
        }
    }
}
