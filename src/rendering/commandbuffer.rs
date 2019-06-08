use std::cell::RefCell;
use std::rc::Rc;
use gfx_hal::{Backend, Device};
use gfx_hal::command;
use gfx_hal::pool;
use gfx_hal::pso;
use gfx_hal::buffer::IndexBufferView;

use super::device::DeviceState;
use super::framebuffer::FramebufferState;
use super::render_pass::RenderPassState;
use super::buffer::{IndexBuffer, VertexBuffer, UniformBuffer};
use super::swapchain::SwapchainState;
use super::pipeline::PipelineState;

pub(super) const MAX_FRAMES_IN_FLIGHT: usize = 2;

pub(super) struct CommandBufferState<B: Backend> {
    device: Rc<RefCell<DeviceState<B>>>,
    command_pool: Option<gfx_hal::CommandPool<B, gfx_hal::Graphics>>,
    pub(super) command_buffers: Option<Vec<command::CommandBuffer<B, gfx_hal::Graphics, command::MultiShot>>>,
    pub(super) acquire_semaphores: Option<Vec<B::Semaphore>>,
    pub(super) present_semaphores: Option<Vec<B::Semaphore>>,
    pub(super) fences: Option<Vec<B::Fence>>
}

impl<B: Backend> CommandBufferState<B> {
    pub(super) unsafe fn new(
        device: Rc<RefCell<DeviceState<B>>>,
        framebuffer_state: &mut FramebufferState<B>,
        render_pass: &RenderPassState<B>,
        swapchain: &SwapchainState<B>,
        pipeline: &PipelineState<B>,
        vertex_buffer: &VertexBuffer<B>,
        index_buffer: &IndexBuffer<B>,
        index_count: u32,
        uniform_buffers: &Vec<UniformBuffer<B>>
    ) -> Self {
        let frame_images = framebuffer_state.frame_images
            .as_ref().unwrap();
        let framebuffers = framebuffer_state.framebuffers
            .as_ref().unwrap();
        let iter_count = if frame_images.len() != 0 {
            frame_images.len()
        } else {
            1 // GL can have zero
        };

        //let mut command_pools: Vec<gfx_hal::CommandPool<B, gfx_hal::Graphics>> = vec![];
        let mut command_pool = device
            .borrow()
            .device
            .create_command_pool_typed(
                &device.borrow().queues,
                pool::CommandPoolCreateFlags::empty(),
            )
            .expect("Can't create command pool");

        let acquire_semaphores: Vec<B::Semaphore> = (0..MAX_FRAMES_IN_FLIGHT)
            .map(|_| device.borrow().device.create_semaphore().unwrap()).collect();
        let present_semaphores: Vec<B::Semaphore> = (0..MAX_FRAMES_IN_FLIGHT)
            .map(|_| device.borrow().device.create_semaphore().unwrap()).collect();
        let fences: Vec<B::Fence> = (0..MAX_FRAMES_IN_FLIGHT)
            .map(|_| device.borrow().device.create_fence(true).unwrap()).collect();

        let mut command_buffers: Vec<command::CommandBuffer<B, gfx_hal::Graphics, command::MultiShot>> = vec![];
        for _ in 0..iter_count {
            let cmd_buffer = command_pool.acquire_command_buffer::<command::MultiShot>();
            command_buffers.push(cmd_buffer);
        }

        for i in 0..iter_count {
            let cmd_buffer = &mut command_buffers[i];
            let framebuffer = &framebuffers[i];
            let uniform_buffer = &uniform_buffers[i];
            cmd_buffer.begin(true);

            {
                let mut encoder = cmd_buffer.begin_render_pass_inline(
                    render_pass.render_pass.as_ref().unwrap(),
                    &framebuffer,
                    pso::Rect {
                        x: 0,
                        y: 0,
                        w: swapchain.extent.width as i16,
                        h: swapchain.extent.height as i16,
                    },
                    &[command::ClearValue::Color(
                        command::ClearColor::Sfloat([
                            0.0, 0.0, 0.0, 1.0,
                        ])
                    ), command::ClearValue::DepthStencil(
                        command::ClearDepthStencil(1.0, 0)
                    )]
                );

                encoder.bind_graphics_pipeline(
                    pipeline.pipeline.as_ref().unwrap());
                encoder.bind_vertex_buffers(0, Some((vertex_buffer.get_buffer(), 0)));
                encoder.bind_index_buffer(IndexBufferView {
                    buffer: index_buffer.get_buffer(),
                    offset: 0,
                    index_type: index_buffer.index_type()
                });
                encoder.bind_graphics_descriptor_sets(
                    pipeline.pipeline_layout.as_ref().unwrap(),
                    0,
                    vec![uniform_buffer.get_descriptor_set()],
                    &[]
                );
                encoder.draw_indexed(0..index_count, 0, 0..1);

                // explicit end_render_pass on Drop
            }

            cmd_buffer.finish();
        }

        CommandBufferState {
            command_pool: Some(command_pool),
            command_buffers: Some(command_buffers),
            acquire_semaphores: Some(acquire_semaphores),
            present_semaphores: Some(present_semaphores),
            fences: Some(fences),
            device
        }
    }
}

impl<B: Backend> Drop for CommandBufferState<B> {
    fn drop(&mut self) {
        let device = &self.device.borrow().device;
        unsafe {
            for fence in self.fences.take().unwrap() {
                device.wait_for_fence(&fence, !0).unwrap();
                device.destroy_fence(fence);
            }

            for acquire_semaphore in self.acquire_semaphores.take().unwrap() {
                device.destroy_semaphore(acquire_semaphore);
            }
            for present_semaphore in self.present_semaphores.take().unwrap() {
                device.destroy_semaphore(present_semaphore);
            }

            #[cfg(feature="vulkan")]
            self.command_pool.as_mut().unwrap()
                .free(self.command_buffers.take().unwrap());

            device.destroy_command_pool(
                self.command_pool.take().unwrap().into_raw());
        }
    }
}
