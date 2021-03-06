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

extern crate image;

use std::cell::RefCell;
use std::rc::Rc;
use std::io::Cursor;

use log::debug;

use gfx_hal::{
    pso,
    queue::Submission,
    Backend,
    Device,
    Instance,
    Swapchain,
};
use gfx_hal::pool::CommandPoolCreateFlags;
#[cfg(feature="gl")]
use gfx_hal::format::{AsFormat, Rgba8Srgb as ColorFormat};

use crate::window::WindowState;
use crate::consts::{APP_TITLE, APP_VERSION};

mod constants;
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
mod descriptors;
mod images;

use adapter::AdapterState;
use device::DeviceState;
use swapchain::SwapchainState;
use render_pass::RenderPassState;
use pipeline::PipelineState;
use framebuffer::FramebufferState;
use commandbuffer::CommandBufferState;
use buffer::{VertexBuffer, IndexBuffer, UniformBuffer};
use descriptors::DescriptorSetLayout;
use images::{DepthImage, Texture, ColorImage};

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
    desc_set_layout: DescriptorSetLayout<B>,
    pipeline: PipelineState<B>,
    framebuffer: FramebufferState<B>,
    vertex_buffer: VertexBuffer<B>,
    index_buffer: IndexBuffer<B>,
    depth_image: DepthImage<B>,
    color_image: ColorImage<B>,
    texture: Texture<B>,
    uniform_desc_pool: Option<B::DescriptorPool>,
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

        let render_pass = RenderPassState::new(
            Rc::clone(&device),
            &backend.adapter,
            swapchain.as_ref().unwrap(),
        );

        let desc_set_layout = DescriptorSetLayout::new(
            Rc::clone(&device),
            vec![
                pso::DescriptorSetLayoutBinding {
                    binding: 0,
                    ty: pso::DescriptorType::UniformBuffer,
                    count: 1,
                    stage_flags: pso::ShaderStageFlags::VERTEX,
                    immutable_samplers: false,
                },
                pso::DescriptorSetLayoutBinding {
                    binding: 1,
                    ty: pso::DescriptorType::CombinedImageSampler,
                    count: 1,
                    stage_flags: pso::ShaderStageFlags::FRAGMENT,
                    immutable_samplers: false
                }
            ]);

        let pipeline = PipelineState::new(
            Rc::clone(&device),
            &backend.adapter,
            vec![desc_set_layout.get_layout()],
            render_pass.render_pass.as_ref().unwrap(),
            swapchain.as_ref().unwrap()
        );

        let mut staging_command_pool = device
            .borrow()
            .device
            .create_command_pool_typed(
                &device.borrow().queues,
                CommandPoolCreateFlags::TRANSIENT,
            )
            .expect("Can't create command pool");

        let color_image = ColorImage::new(
            Rc::clone(&device),
            &backend.adapter,
            swapchain.as_ref().unwrap(),
            &mut staging_command_pool
        );


        let depth_image = {
            let width = swapchain.as_ref().unwrap().extent.width;
            let height = swapchain.as_ref().unwrap().extent.height;
            DepthImage::new(
                Rc::clone(&device),
                &backend.adapter,
                width, height,
                &mut staging_command_pool
            )
        };

        let mut framebuffer = FramebufferState::new(
            Rc::clone(&device),
            &render_pass,
            swapchain.as_mut().unwrap(),
            &color_image,
            &depth_image
        );

        let model = primitives::Model::load(std::path::Path::new("models/chalet.obj"));

        let vertex_buffer = VertexBuffer::new::<primitives::Vertex>(
            Rc::clone(&device),
            &mut staging_command_pool,
            &model.vertices,
            &backend.adapter.memory_types,
        );

        let index_buffer = IndexBuffer::new(
            Rc::clone(&device),
            &mut staging_command_pool,
            &model.indicies,
            &backend.adapter.memory_types
        );

        let img = image::load(Cursor::new(&images::CHALET_JPG_DATA[..]), image::JPEG)
            .unwrap()
            .to_rgba();

        let texture = Texture::new(
            Rc::clone(&device),
            &backend.adapter,
            &mut staging_command_pool,
            &img
        );

        // TODO: all this in one constructor

        let num_buffers = framebuffer.framebuffers.as_ref().unwrap().len();

        let mut uniform_desc_pool = device
            .borrow()
            .device
            .create_descriptor_pool(
                num_buffers,
                &[pso::DescriptorRangeDesc {
                    ty: pso::DescriptorType::UniformBuffer,
                    count: num_buffers,
                }, pso::DescriptorRangeDesc {
                    ty: pso::DescriptorType::CombinedImageSampler,
                    count: num_buffers
                }],
                pso::DescriptorPoolCreateFlags::empty(),
            )
            .ok();

        let uniform_desc_sets = desc_set_layout.create_desc_sets(
            uniform_desc_pool.as_mut().unwrap(),
            num_buffers
        );
        let mut uniform_buffers = Vec::with_capacity(num_buffers);
        for desc in uniform_desc_sets {
            texture.write_descriptor_set(
                &mut device.borrow_mut().device,
                &desc,
                1
            );

            let ub = UniformBuffer::new::<primitives::UniformBufferObject>(
                Rc::clone(&device),
                &backend.adapter.memory_types,
                desc,
                0
            );
            uniform_buffers.push(ub);
        }

        device.borrow().device.destroy_command_pool(
            staging_command_pool.into_raw());

        // ------------------

        let commandbuffer = CommandBufferState::new(
            Rc::clone(&device),
            &mut framebuffer,
            &render_pass,
            swapchain.as_ref().unwrap(),
            &pipeline,
            &vertex_buffer,
            &index_buffer,
            model.indicies.len() as _,
            &uniform_buffers
        );

        let viewport = RendererState::create_viewport(
            swapchain.as_ref().unwrap());

        RendererState {
            device,
            swapchain,
            backend,
            window,
            render_pass,
            desc_set_layout,
            pipeline,
            framebuffer,
            vertex_buffer,
            index_buffer,
            depth_image,
            color_image,
            texture,
            uniform_desc_pool,
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
            RenderPassState::new(
                Rc::clone(&self.device),
                &self.backend.adapter,
                self.swapchain.as_ref().unwrap()
            )
        };

        let mut staging_command_pool = unsafe {
            self.device
                .borrow()
                .device
                .create_command_pool_typed(
                    &self.device.borrow().queues,
                    CommandPoolCreateFlags::TRANSIENT,
                )
                .expect("Can't create command pool")
        };

        self.color_image = unsafe {
            ColorImage::new(
                Rc::clone(&self.device),
                &self.backend.adapter,
                self.swapchain.as_ref().unwrap(),
                &mut staging_command_pool
            )
        };

        self.depth_image = unsafe {
            let width = self.swapchain.as_ref().unwrap().extent.width;
            let height = self.swapchain.as_ref().unwrap().extent.height;
            DepthImage::new(
                Rc::clone(&self.device),
                &self.backend.adapter,
                width, height,
                &mut staging_command_pool
            )
        };

        self.framebuffer = unsafe {
            FramebufferState::new(
                Rc::clone(&self.device),
                &self.render_pass,
                self.swapchain.as_mut().unwrap(),
                &self.color_image,
                &self.depth_image
            )
        };

        self.desc_set_layout = unsafe {
            DescriptorSetLayout::new(
            Rc::clone(&self.device),
            vec![
                pso::DescriptorSetLayoutBinding {
                    binding: 0,
                    ty: pso::DescriptorType::UniformBuffer,
                    count: 1,
                    stage_flags: pso::ShaderStageFlags::VERTEX,
                    immutable_samplers: false,
                },
                pso::DescriptorSetLayoutBinding {
                    binding: 1,
                    ty: pso::DescriptorType::CombinedImageSampler,
                    count: 1,
                    stage_flags: pso::ShaderStageFlags::FRAGMENT,
                    immutable_samplers: false
                }
            ])
        };

        self.pipeline = unsafe {
            PipelineState::new(
                Rc::clone(&self.device),
                &self.backend.adapter,
                vec![self.desc_set_layout.get_layout()],
                self.render_pass.render_pass.as_ref().unwrap(),
                self.swapchain.as_mut().unwrap(),
            )
        };

        let model = primitives::Model::load(std::path::Path::new("models/chalet.obj"));

        self.vertex_buffer = unsafe {
            VertexBuffer::new::<primitives::Vertex>(
                Rc::clone(&self.device),
                &mut staging_command_pool,
                &model.vertices,
                &self.backend.adapter.memory_types,
            )
        };

        self.index_buffer = unsafe {
            IndexBuffer::new(
                Rc::clone(&self.device),
                &mut staging_command_pool,
                &model.indicies,
                &self.backend.adapter.memory_types,
            )
        };

        let img = image::load(Cursor::new(&images::CHALET_JPG_DATA[..]), image::JPEG)
            .unwrap()
            .to_rgba();

        self.texture = unsafe {
            Texture::new(
                Rc::clone(&self.device),
                &self.backend.adapter,
                &mut staging_command_pool,
                &img
            )
        };

        unsafe {
            self.device.borrow().device.destroy_command_pool(
                staging_command_pool.into_raw());
            self.device
                .borrow()
                .device
                .destroy_descriptor_pool(self.uniform_desc_pool.take().unwrap());

            let num_buffers = self.framebuffer.framebuffers.as_ref().unwrap().len();
            self.uniform_desc_pool = self.device
                .borrow()
                .device
                .create_descriptor_pool(
                    num_buffers,
                    &[pso::DescriptorRangeDesc {
                        ty: pso::DescriptorType::UniformBuffer,
                        count: num_buffers,
                    }, pso::DescriptorRangeDesc {
                        ty: pso::DescriptorType::CombinedImageSampler,
                        count: num_buffers
                    }],
                    pso::DescriptorPoolCreateFlags::empty(),
                )
                .ok();

            let uniform_desc_sets = self.desc_set_layout.create_desc_sets(
                self.uniform_desc_pool.as_mut().unwrap(),
                num_buffers
            );

            self.uniform_buffers = uniform_desc_sets
                .into_iter()
                .map(|desc| {
                    self.texture.write_descriptor_set(&mut self.device.borrow_mut().device, &desc, 1);
                    UniformBuffer::new::<primitives::UniformBufferObject>(
                        Rc::clone(&self.device),
                        &self.backend.adapter.memory_types,
                        desc,
                        0
                    )
                }).collect();
        }

        self.commandbuffer = unsafe {
            CommandBufferState::new(
                Rc::clone(&self.device),
                &mut self.framebuffer,
                &self.render_pass,
                self.swapchain.as_ref().unwrap(),
                &self.pipeline,
                &self.vertex_buffer,
                &self.index_buffer,
                model.indicies.len() as _,
                &self.uniform_buffers
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
        let time: f32 = utils::as_float_secs(&start_time.elapsed()) / 2.0;
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
        let mut ubo = primitives::UniformBufferObject {
            model: glm::rotate(
                &glm::Mat4::identity(),
                time * rad90,
                &glm::vec3(0.0, 0.0, 1.0)),
            view: glm::look_at(
                &glm::vec3(2.0, 2.0, 2.0),
                &glm::vec3(0.0, 0.0, 0.0),
                &glm::vec3(0.0, 0.0, 1.0)),
            proj: glm::perspective(
                utils::ratio(swapchain_extent.width, swapchain_extent.height),
                rad45,
                0.1,
                10.0)
        };
        ubo.proj[1 * 4 + 1] *= -1.0;

        let uniform_buffer = &mut self.uniform_buffers[frame as usize];
        uniform_buffer.update_data(0, &[ubo]);

        unsafe {

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

impl<B: Backend> Drop for RendererState<B> {
    fn drop(&mut self) {
        self.device.borrow().device.wait_idle().unwrap();
        unsafe {
            self.device
                .borrow()
                .device
                .destroy_descriptor_pool(self.uniform_desc_pool.take().unwrap());
            self.swapchain.take();
        }
    }
}
