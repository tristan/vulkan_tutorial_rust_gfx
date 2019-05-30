use std::cell::RefCell;
use std::rc::Rc;
use gfx_hal::{Backend, Device, Primitive};
use gfx_hal::pso;
use super::device::DeviceState;
use super::swapchain::SwapchainState;
use super::primitives;

include!(concat!(env!("OUT_DIR"), "/compiled_shaders.rs"));

const ENTRY_NAME: &str = "main";

pub(super) struct PipelineState<B: Backend> {
    pub(super) pipeline: Option<B::GraphicsPipeline>,
    pipeline_layout: Option<B::PipelineLayout>,
    device: Rc<RefCell<DeviceState<B>>>
}

impl<B: Backend> PipelineState<B> {
    pub(super) unsafe fn new(
        device_ptr: Rc<RefCell<DeviceState<B>>>,
        render_pass: &B::RenderPass,
        swapchain: &SwapchainState<B>,
    ) -> Self {
        let device = &device_ptr.borrow().device;

        let desc_layouts = Vec::<B::DescriptorSetLayout>::new();

        let pipeline_layout = device
            .create_pipeline_layout(desc_layouts, &[])
            .expect("Can't create pipeline layout");

        let pipeline = {

            let vs_module = device.create_shader_module(
                &TRIANGLE_VERTEX_SHADER).unwrap();
            let fs_module = device.create_shader_module(
                &TRIANGLE_FRAGMENT_SHADER).unwrap();

            let pipeline = {

                let (vs_entry, fs_entry) = (
                    pso::EntryPoint::<B> {
                        entry: ENTRY_NAME,
                        module: &vs_module,
                        specialization: pso::Specialization::default(),
                    },
                    pso::EntryPoint::<B> {
                        entry: ENTRY_NAME,
                        module: &fs_module,
                        specialization: pso::Specialization::default(),
                    },
                );

                let shader_entries = pso::GraphicsShaderSet {
                    vertex: vs_entry,
                    hull: None,
                    domain: None,
                    geometry: None,
                    fragment: Some(fs_entry),
                };

                let subpass = gfx_hal::pass::Subpass {
                    index: 0,
                    main_pass: render_pass,
                };

                let mut pipeline_desc = pso::GraphicsPipelineDesc::new(
                    shader_entries,
                    Primitive::TriangleList,
                    pso::Rasterizer {
                        polygon_mode: pso::PolygonMode::Fill,
                        cull_face: pso::Face::BACK,
                        front_face: pso::FrontFace::Clockwise,
                        depth_clamping: false,
                        depth_bias: None,
                        conservative: false,
                    },
                    &pipeline_layout,
                    subpass,
                );

                pipeline_desc.baked_states.viewport = Some(pso::Viewport {
                    rect: pso::Rect {
                        x: 0,
                        y: 0,
                        w: swapchain.extent.width as _,
                        h: swapchain.extent.height as _,
                    },
                    depth: 0.0..1.0
                });

                pipeline_desc.baked_states.scissor = Some(pso::Rect {
                    x: 0,
                    y: 0,
                    w: swapchain.extent.width as _,
                    h: swapchain.extent.height as _,
                });

                pipeline_desc.blender.targets.push(pso::ColorBlendDesc(
                    pso::ColorMask::ALL,
                    pso::BlendState::ADD
                ));

                pipeline_desc.vertex_buffers.push(
                    primitives::Vertex::BINDING_DESCRIPTION
                );

                pipeline_desc.attributes.extend_from_slice(
                    &primitives::Vertex::ATTRIBUTE_DESCRIPTIONS
                );

                device.create_graphics_pipeline(&pipeline_desc, None)
            };

            device.destroy_shader_module(vs_module);
            device.destroy_shader_module(fs_module);

            pipeline.unwrap()
        };

        PipelineState {
            pipeline: Some(pipeline),
            pipeline_layout: Some(pipeline_layout),
            device: Rc::clone(&device_ptr)
        }
    }
}

impl<B: Backend> Drop for PipelineState<B> {
    fn drop(&mut self) {
        let device = &self.device.borrow().device;
        unsafe {
            device.destroy_graphics_pipeline(self.pipeline.take().unwrap());
            device.destroy_pipeline_layout(self.pipeline_layout.take().unwrap());
            // implicit destroy pipeline on Drop
        }
    }
}
