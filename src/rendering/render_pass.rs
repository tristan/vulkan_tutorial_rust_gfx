use std::cell::RefCell;
use std::rc::Rc;
use gfx_hal::{Backend, Device};
use gfx_hal::image;
use gfx_hal::image::Layout;
use gfx_hal::pass;
use gfx_hal::pso;
use super::device::DeviceState;
use super::swapchain::SwapchainState;

pub(super) struct RenderPassState<B: Backend> {
    pub(super) render_pass: Option<B::RenderPass>,
    device: Rc<RefCell<DeviceState<B>>>,
}

impl<B: Backend> RenderPassState<B> {
    pub(super) unsafe fn new(
        device: Rc<RefCell<DeviceState<B>>>,
        swapchain: &SwapchainState<B>
    ) -> Self {
        let render_pass = {
            let color_attachment = pass::Attachment {
                format: Some(swapchain.format.clone()),
                samples: 1,
                ops: pass::AttachmentOps::new(
                    pass::AttachmentLoadOp::Clear,
                    pass::AttachmentStoreOp::Store,
                ),
                stencil_ops: pass::AttachmentOps::DONT_CARE,
                layouts: Layout::Undefined..Layout::Present
            };

            let depth_attachment = pass::Attachment {
                format: device.borrow().optimal_depth_format(),
                samples: 1,
                ops: pass::AttachmentOps::new(
                    pass::AttachmentLoadOp::Clear,
                    pass::AttachmentStoreOp::DontCare,
                ),
                stencil_ops: pass::AttachmentOps::DONT_CARE,
                layouts: Layout::Undefined..Layout::DepthStencilAttachmentOptimal
            };

            let subpass = pass::SubpassDesc {
                colors: &[(0, Layout::ColorAttachmentOptimal)],
                depth_stencil: Some(&(1, Layout::DepthStencilAttachmentOptimal)),
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };

            let dependency = pass::SubpassDependency {
                passes: pass::SubpassRef::External..
                    pass::SubpassRef::Pass(0),
                stages: pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT..
                    pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                accesses: image::Access::empty()..
                    (image::Access::COLOR_ATTACHMENT_READ |
                     image::Access::COLOR_ATTACHMENT_WRITE)
            };

            device
                .borrow()
                .device
                .create_render_pass(&[color_attachment, depth_attachment],
                                    &[subpass],
                                    &[dependency])
                .ok()
        };

        RenderPassState {
            render_pass,
            device
        }
    }
}

impl<B: Backend> Drop for RenderPassState<B> {
    fn drop(&mut self) {
        let device = &self.device.borrow().device;
        unsafe {
            device.destroy_render_pass(self.render_pass.take().unwrap());
        }
    }
}
