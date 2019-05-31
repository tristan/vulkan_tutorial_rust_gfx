#[cfg(feature = "dx11")]
extern crate gfx_backend_dx11 as back;
#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12 as back;
#[cfg(not(any(
    feature = "vulkan",
    feature = "dx11",
    feature = "dx12",
    feature = "metal",
    feature = "gl"
)))]
extern crate gfx_backend_empty as back;
#[cfg(feature = "gl")]
extern crate gfx_backend_gl as back;
#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;

use std::cell::RefCell;
use std::rc::Rc;

use log::debug;

use gfx_hal::{
    pso,
    queue::Submission,
    Backend,
    Device,
    Instance,
    Swapchain,
};
#[cfg(feature="gl")]
use gfx_hal::format::{AsFormat, Rgba8Srgb as ColorFormat};

use crate::window::WindowState;
use crate::consts::{APP_TITLE, APP_VERSION};

mod utils;
mod primitives;
mod adapter;
mod device;
mod swapchain;
mod render_pass;
mod pipeline;
mod framebuffer;
mod commandbuffer;
mod buffer;
mod descriptor_set_layout;

use adapter::AdapterState;
use device::DeviceState;
use swapchain::SwapchainState;
use render_pass::RenderPassState;
use pipeline::PipelineState;
use framebuffer::FramebufferState;
use commandbuffer::CommandBufferState;
use buffer::{VertexBuffer, IndexBuffer, UniformBuffer};
use descriptor_set_layout::DescriptorSetLayout;

pub struct BackendState<B: Backend> {
    surface: B::Surface,
    adapter: AdapterState<B>,
    #[cfg(any(feature = "vulkan", feature = "dx11", feature = "dx12", feature = "metal"))]
    #[allow(dead_code)]
    window: winit::Window,
}

#[cfg(any(feature = "vulkan", feature = "dx11", feature = "dx12", feature = "metal"))]
pub fn create_backend(window_state: &mut WindowState) -> (BackendState<back::Backend>, back::Instance) {
    let window = window_state
        .wb
        .take()
        .unwrap()
        .build(&window_state.events_loop)
        .unwrap();

    let instance = back::Instance::create(APP_TITLE, APP_VERSION);
    let mut adapters = instance.enumerate_adapters();
    let adapter = AdapterState::new(&mut adapters);
    let surface = instance.create_surface(&window);
    (BackendState {adapter, surface, window}, instance)
}

#[cfg(feature = "gl")]
pub fn create_backend(window_state: &mut WindowState) -> (BackendState<back::Backend>, ()) {
    let window = {
        let builder =
            back::config_context(back::glutin::ContextBuilder::new(),
                                 ColorFormat::SELF,
                                 None)
                .with_vsync(true);
        back::glutin::WindowedContext::new_windowed(
            window_state.wb.take().unwrap(),
            builder,
            &window_state.events_loop,
        ).unwrap()
    };

    let surface = back::Surface::from_window(window);
    let mut adapters = surface.enumerate_adapters();
    (
        BackendState {
            adapter: AdapterState::new(&mut adapters),
            surface
        },
        (),
    )
}

pub struct RendererState<B: Backend> {
    device: Rc<RefCell<DeviceState<B>>>,
    swapchain: Option<SwapchainState<B>>,
    backend: BackendState<B>,
    window: WindowState,
    render_pass: RenderPassState<B>,
    descriptor_set_layout: DescriptorSetLayout<B>,
    pipeline: PipelineState<B>,
    framebuffer: FramebufferState<B>,
    vertex_buffer: VertexBuffer<B>,
    index_buffer: IndexBuffer<B>,
    uniform_buffers: Vec<UniformBuffer<B>>,
    commandbuffer: CommandBufferState<B>,
    viewport: pso::Viewport,
}

impl<B: Backend> RendererState<B> {

    pub unsafe fn new(mut backend: BackendState<B>, window: WindowState) -> Self {
        let device = Rc::new(RefCell::new(DeviceState::new(
            backend.adapter.adapter.take().unwrap(),
            &backend.surface,
        )));

        let mut swapchain = Some(SwapchainState::new(&mut backend, Rc::clone(&device)));

        let render_pass = RenderPassState::new(swapchain.as_ref().unwrap(), Rc::clone(&device));

        let descriptor_set_layout = DescriptorSetLayout::new(
            Rc::clone(&device),
            vec![
                pso::DescriptorSetLayoutBinding {
                    binding: 0,
                    ty: pso::DescriptorType::UniformBuffer,
                    count: 1,
                    stage_flags: pso::ShaderStageFlags::VERTEX,
                    immutable_samplers: false,
                }
            ]);

        let pipeline = PipelineState::new(
            Rc::clone(&device),
            vec![descriptor_set_layout.get_layout()],
            render_pass.render_pass.as_ref().unwrap(),
            swapchain.as_ref().unwrap()
        );

        let mut framebuffer = FramebufferState::new(
            Rc::clone(&device),
            &render_pass,
            swapchain.as_mut().unwrap(),
        );

        let vertex_buffer = VertexBuffer::new::<primitives::Vertex>(
            Rc::clone(&device),
            &primitives::VERTICIES,
            &backend.adapter.memory_types,
        );

        let index_buffer = IndexBuffer::new::<u16>(
            Rc::clone(&device),
            &primitives::INDICIES,
            &backend.adapter.memory_types
        );

        let num_buffers = framebuffer.framebuffers.as_ref().unwrap().len();
        let uniform_buffers = (0..num_buffers).map(|_| {
            UniformBuffer::new::<primitives::UniformBufferObject>(
                Rc::clone(&device),
                &backend.adapter.memory_types
            )
        }).collect();

        let commandbuffer = CommandBufferState::new(
            Rc::clone(&device),
            &mut framebuffer,
            &render_pass,
            swapchain.as_ref().unwrap(),
            &pipeline,
            &vertex_buffer,
            &index_buffer,
            primitives::INDICIES.len() as _
        );

        let viewport = RendererState::create_viewport(
            swapchain.as_ref().unwrap());

        RendererState {
            device,
            swapchain,
            backend,
            window,
            render_pass,
            descriptor_set_layout,
            pipeline,
            framebuffer,
            vertex_buffer,
            index_buffer,
            uniform_buffers,
            commandbuffer,
            viewport
        }
    }

    fn recreate_swapchain(&mut self) {
        self.device.borrow().device.wait_idle().unwrap();

        self.swapchain.take().unwrap();

        self.swapchain =
            Some(unsafe { SwapchainState::new(&mut self.backend, Rc::clone(&self.device)) });

        self.render_pass = unsafe {
            RenderPassState::new(self.swapchain.as_ref().unwrap(), Rc::clone(&self.device))
        };

        self.framebuffer = unsafe {
            FramebufferState::new(
                Rc::clone(&self.device),
                &self.render_pass,
                self.swapchain.as_mut().unwrap(),
            )
        };

        self.descriptor_set_layout = unsafe {
            DescriptorSetLayout::new(
            Rc::clone(&self.device),
            vec![
                pso::DescriptorSetLayoutBinding {
                    binding: 0,
                    ty: pso::DescriptorType::UniformBuffer,
                    count: 1,
                    stage_flags: pso::ShaderStageFlags::VERTEX,
                    immutable_samplers: false,
                }
            ])
        };

        self.pipeline = unsafe {
            PipelineState::new(
                Rc::clone(&self.device),
                vec![self.descriptor_set_layout.get_layout()],
                self.render_pass.render_pass.as_ref().unwrap(),
                self.swapchain.as_mut().unwrap(),
            )
        };

        self.vertex_buffer = unsafe {
            VertexBuffer::new::<primitives::Vertex>(
                Rc::clone(&self.device),
                &primitives::VERTICIES,
                &self.backend.adapter.memory_types,
            )
        };


        self.index_buffer = unsafe {
            IndexBuffer::new::<u16>(
                Rc::clone(&self.device),
                &primitives::INDICIES,
                &self.backend.adapter.memory_types,
            )
        };

        self.uniform_buffers = unsafe {
            let num_buffers = self.framebuffer.framebuffers.as_ref().unwrap().len();
            (0..num_buffers).map(|_| {
                UniformBuffer::new::<primitives::UniformBufferObject>(
                    Rc::clone(&self.device),
                    &self.backend.adapter.memory_types
                )
            }).collect()
        };

        self.commandbuffer = unsafe {
            CommandBufferState::new(
                Rc::clone(&self.device),
                &mut self.framebuffer,
                &self.render_pass,
                self.swapchain.as_ref().unwrap(),
                &self.pipeline,
                &self.vertex_buffer,
                &self.index_buffer,
                primitives::INDICIES.len() as _
            )
        };

        self.viewport = RendererState::create_viewport(
            self.swapchain.as_ref().unwrap());
    }

    fn create_viewport(swapchain: &SwapchainState<B>) -> pso::Viewport {
        pso::Viewport {
            rect: pso::Rect {
                x: 0,
                y: 0,
                w: swapchain.extent.width as i16,
                h: swapchain.extent.height as i16,
            },
            depth: 0.0..1.0,
        }
    }

    fn draw_frame(&mut self, start_time: &std::time::Instant, frame_number: usize) -> bool {
        let current_frame = frame_number % commandbuffer::MAX_FRAMES_IN_FLIGHT;
        let acquire_semaphore = &self.commandbuffer
            .acquire_semaphores.as_ref().unwrap()[current_frame];
        let present_semaphore = &self.commandbuffer
            .present_semaphores.as_ref().unwrap()[current_frame];
        let fence = &self.commandbuffer
            .fences.as_ref().unwrap()[current_frame];

        unsafe {
            let device = &self.device.borrow().device;
            device.wait_for_fence(&fence, !0).unwrap();
        }

        let frame: gfx_hal::SwapImageIndex = unsafe {
            match self.swapchain
                .as_mut()
                .unwrap()
                .swapchain
                .as_mut()
                .unwrap()
                .acquire_image(
                    !0,
                    Some(&acquire_semaphore),
                    None)
            {
                Ok((i, _)) => i,
                Err(e) => {
                    match e {
                        gfx_hal::AcquireError::OutOfDate =>
                            return false,
                        _ => ()
                    };
                    panic!(e)
                },
            }
        };

        let current_cmd_buffer = &self.commandbuffer.command_buffers.as_ref().unwrap()[frame as usize];
        let submission = Submission {
            command_buffers: std::iter::once(current_cmd_buffer),
            wait_semaphores: std::iter::once((&acquire_semaphore, pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT)),
            signal_semaphores: std::iter::once(&present_semaphore)
        };

        // update UBO
        let time: f32 = utils::as_float_secs(&start_time.elapsed());
        let swapchain_extent = &self.swapchain
            .as_ref()
            .unwrap()
            .extent;
        let rad90 = {
            glm::radians(&glm::vec1(90.0))[0]
        };
        let rad45 = {
            glm::radians(&glm::vec1(45.0))[0]
        };
        let ubo = primitives::UniformBufferObject {
            model: glm::rotate(
                &glm::make_mat4(&[1.0]),
                rad90 * time,
                &glm::vec3(0.0, 0.0, 1.0)),
            view: glm::look_at(
                &glm::vec3(2.0, 2.0, 2.0),
                &glm::vec3(0.0, 0.0, 0.0),
                &glm::vec3(0.0, 0.0, 1.0)),
            proj: glm::perspective_lh(
                utils::ratio(swapchain_extent.width, swapchain_extent.height),
                rad45,
                0.1,
                10.0)
        };
        let uniform_buffer = &mut self.uniform_buffers[frame as usize];

        unsafe {
            uniform_buffer.update_data(&[&ubo]);

            {
                let device = &self.device.borrow().device;
                device.reset_fence(&fence).unwrap();
            }

            let queue = &mut self.device.borrow_mut().queues.queues[0];
            queue.submit(submission, Some(fence));

            match self
                .swapchain
                .as_ref()
                .unwrap()
                .swapchain
                .as_ref()
                .unwrap()
                .present(
                    queue,
                    frame,
                    Some(&present_semaphore)
                )
            {
                Ok(suboptimal) => {
                    if suboptimal.is_some() {
                        return false;
                    }
                },
                Err(e) => {
                    match e {
                        gfx_hal::window::PresentError::OutOfDate =>
                            return false,
                        _ => ()
                    };
                    panic!(e)
                }
            }
        }

        return true;
    }

    pub fn mainloop(&mut self) {
        let mut running = true;
        let mut frame_number = 0;
        let start_time = std::time::Instant::now();
        while running {
            self.window.events_loop.poll_events(|event| {
                if let winit::Event::WindowEvent { event, .. } = event {
                    #[allow(unused_variables)]
                    match event {
                        winit::WindowEvent::KeyboardInput {
                            input:
                            winit::KeyboardInput {
                                virtual_keycode: Some(winit::VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                        }
                        | winit::WindowEvent::CloseRequested => running = false,
                        winit::WindowEvent::Resized(dims) => {
                            // TODO: is this the same as glfwSetFramebufferSizeCallback ?
                            // Do we need 'Handling resizes explicitly' section?
                            debug!("RESIZED {:?}", dims);
                        },
                        _ => (),
                    }
                }
            });
            if self.draw_frame(&start_time, frame_number) == false {
                self.recreate_swapchain();
                continue;
            };
            frame_number += 1;
            if frame_number % 60 == 0 {
                println!("...");
            }
        }
        self.device.borrow().device.wait_idle().unwrap();
    }
}
