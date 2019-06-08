use std::cell::RefCell;
use std::rc::Rc;

use gfx_hal::{Backend, Device, CommandPool, Graphics};
use gfx_hal::buffer::Usage as BufferUsage;
use gfx_hal::image::{Access, Layout, Usage as ImageUsage,
                     Kind, Size, SubresourceLayers, Tiling,
                     ViewCapabilities, Offset, Extent, ViewKind,
                     SamplerInfo, Filter, WrapMode, Anisotropic,
                     Lod, SubresourceRange};
use gfx_hal::format::{AsFormat, Format, Aspects, Rgba8Unorm, Swizzle, ImageFeature
};
use gfx_hal::memory::{Barrier, Properties as MemoryProperties, Dependencies as MemoryDependencies};
use gfx_hal::command;
use gfx_hal::pso;
use gfx_hal::pso::PipelineStage;

use image;

use super::adapter::AdapterState;
use super::device::DeviceState;
use super::swapchain::SwapchainState;
use super::descriptors::DescriptorSet;
use super::buffer::TextureBuffer;

pub(super) const CHALET_JPG_DATA: &'static [u8] = include_bytes!("../../textures/chalet.jpg");

unsafe fn create_image<B: Backend>(
    device: &B::Device, adapter: &AdapterState<B>, kind: Kind,
    format: Format, tiling: Tiling, usage: ImageUsage,
    properties: MemoryProperties, mip_levels: u8
) -> (B::Image, B::Memory) {
    let mut image = device
        .create_image(
            kind, // kind
            mip_levels,  // mip_levels
            format, // format
            tiling, // tiling
            usage, // usage
            ViewCapabilities::empty() // view_capabilities
        )
        .unwrap();

    let mem_req = device.get_image_requirements(&image);

    let device_type = adapter
        .memory_types
        .iter()
        .enumerate()
        .position(|(id, memory_type)| {
            mem_req.type_mask & (1 << id) != 0
                && memory_type.properties.contains(
                    properties)
        })
        .unwrap()
        .into();

    let memory = device.allocate_memory(device_type, mem_req.size).unwrap();
    device.bind_image_memory(&memory, 0, &mut image).unwrap();

    (image, memory)
}


pub(super) struct Texture<B: Backend> {
    device: Rc<RefCell<DeviceState<B>>>,
    memory: Option<B::Memory>,
    image: Option<B::Image>,
    image_view: Option<B::ImageView>,
    sampler: Option<B::Sampler>,
}

impl<B: Backend> Texture<B> {
    pub(super) unsafe fn new(
        device_ptr: Rc<RefCell<DeviceState<B>>>,
        adapter: &AdapterState<B>,
        command_pool: &mut CommandPool<B, Graphics>,
        img: &image::ImageBuffer<image::Rgba<u8>, Vec<u8>>
    ) -> Self {

        let props = device_ptr.borrow().format_properties(Some(Rgba8Unorm::SELF));
        if props.optimal_tiling & ImageFeature::SAMPLED_LINEAR != ImageFeature::SAMPLED_LINEAR {
            panic!("texture image format does not support linear blitting!");
        }

        let (buffer, width, height, row_pitch, stride) = {
            TextureBuffer::new(
                Rc::clone(&device_ptr),
                &adapter,
                &img,
                BufferUsage::TRANSFER_SRC
            )
        };

        let mip_levels = (std::cmp::max(width, height) as f64)
            .log2()
            .floor() as u8 + 1;

        let (image, memory) = create_image(
            &device_ptr.borrow().device,
            &adapter,
            Kind::D2(width as Size, height as Size, 1, 1),
            Rgba8Unorm::SELF,
            Tiling::Optimal,
            ImageUsage::TRANSFER_SRC | ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
            MemoryProperties::DEVICE_LOCAL,
            mip_levels
        );

        let subresourcerange = SubresourceRange {
            aspects: Aspects::COLOR,
            levels: 0..mip_levels,
            layers: 0..1,
        };

        // copy buffer to texture
        {
            let mut cmd_buffer = command_pool.acquire_command_buffer::<command::OneShot>();
            cmd_buffer.begin();

            let image_barrier = Barrier::Image {
                states: (Access::empty(), Layout::Undefined)
                    ..(Access::TRANSFER_WRITE, Layout::TransferDstOptimal),
                target: &image,
                families: None,
                range: subresourcerange.clone(),
            };

            cmd_buffer.pipeline_barrier(
                PipelineStage::TOP_OF_PIPE..PipelineStage::TRANSFER,
                MemoryDependencies::empty(),
                &[image_barrier]
            );

            cmd_buffer.copy_buffer_to_image(
                (&buffer).get_buffer(),
                &image,
                Layout::TransferDstOptimal,
                &[command::BufferImageCopy { // region
                    buffer_offset: 0,
                    buffer_width: row_pitch / (stride as u32), // buffer_row_length
                    buffer_height: height, // buffer_image_height
                    image_layers: SubresourceLayers { // image_subresource
                        aspects: Aspects::COLOR,
                        level: 0,
                        layers: 0..1
                    },
                    image_offset: Offset { x: 0, y: 0, z: 0 },
                    image_extent: Extent {
                        width,
                        height,
                        depth: 1
                    }

                }]
            );

            let mut src_mip_width = width;
            let mut src_mip_height = height;

            for i in 1..mip_levels {
                let image_barrier = Barrier::Image {
                    states: (Access::TRANSFER_WRITE, Layout::TransferDstOptimal)
                        ..(Access::TRANSFER_READ, Layout::TransferSrcOptimal),
                    target: &image,
                    families: None,
                    range: SubresourceRange {
                        aspects: Aspects::COLOR,
                        levels: (i - 1)..i,
                        layers: 0..1,
                    }
                };

                cmd_buffer.pipeline_barrier(
                    PipelineStage::TRANSFER..PipelineStage::TRANSFER,
                    MemoryDependencies::empty(),
                    &[image_barrier],
                );


                let dst_mip_width = {
                    if src_mip_width > 1 {
                        src_mip_width / 2
                    } else {
                        1
                    }
                };
                let dst_mip_height = {
                    if src_mip_height > 1 {
                        src_mip_height / 2
                    } else {
                        1
                    }
                };

                cmd_buffer.blit_image(
                    &image, // image
                    Layout::TransferSrcOptimal, // src_layout
                    &image, // dst
                    Layout::TransferDstOptimal, // dst_layout
                    Filter::Linear, // filter,
                    Some(command::ImageBlit {
                        src_subresource: SubresourceLayers {
                            aspects: Aspects::COLOR,
                            level: i - 1,
                            layers: 0..1
                        },
                        src_bounds: Offset { x: 0, y: 0, z: 0 }..
                            Offset { x: src_mip_width as _, y: src_mip_height as _, z: 1 },
                        dst_subresource: SubresourceLayers {
                            aspects: Aspects::COLOR,
                            level: i,
                            layers: 0..1
                        },
                        dst_bounds: Offset { x: 0, y: 0, z: 0 }..
                            Offset { x: dst_mip_width as _, y: dst_mip_height as _, z: 1 }
                    })
                );

                let image_barrier = Barrier::Image {
                    states: (Access::TRANSFER_READ, Layout::TransferSrcOptimal)
                        ..(Access::SHADER_READ, Layout::ShaderReadOnlyOptimal),
                    target: &image,
                    families: None,
                    range: SubresourceRange {
                        aspects: Aspects::COLOR,
                        levels: (i - 1)..i,
                        layers: 0..1,
                    }
                };

                cmd_buffer.pipeline_barrier(
                    PipelineStage::TRANSFER..PipelineStage::FRAGMENT_SHADER,
                    MemoryDependencies::empty(),
                    &[image_barrier],
                );

                src_mip_width = dst_mip_width;
                src_mip_height = dst_mip_height;
            }

            let image_barrier = Barrier::Image {
                states: (Access::TRANSFER_WRITE, Layout::TransferDstOptimal)
                    ..(Access::SHADER_READ, Layout::ShaderReadOnlyOptimal),
                target: &image,
                families: None,
                range: SubresourceRange {
                    aspects: Aspects::COLOR,
                    levels: (mip_levels - 1)..mip_levels,
                    layers: 0..1,
                }
            };

            cmd_buffer.pipeline_barrier(
                PipelineStage::TRANSFER..PipelineStage::FRAGMENT_SHADER,
                MemoryDependencies::empty(),
                &[image_barrier],
            );

            cmd_buffer.finish();

            let queue = &mut device_ptr.borrow_mut().queues.queues[0];
            queue.submit_without_semaphores(std::iter::once(&cmd_buffer), None);
            queue.wait_idle().unwrap();
        }

        let (image_view, sampler) = {
            let device = &device_ptr.borrow().device;

            let image_view = device
                .create_image_view(
                    &image,
                    ViewKind::D2,
                    Rgba8Unorm::SELF,
                    Swizzle::NO,
                    subresourcerange
                )
                .unwrap();

            let mut sampler_info = SamplerInfo::new(Filter::Linear, WrapMode::Tile);
            sampler_info.anisotropic = Anisotropic::On(16);
            let lod0: Lod = 0.0f32.into();
            let lodn: Lod = (mip_levels as f32).into();
            sampler_info.lod_range = lod0..lodn;
            let sampler = device
                .create_sampler(sampler_info) // TILE = REPEAT
                .expect("Can't create sampler");

            (image_view, sampler)
        };

        Texture {
            device: device_ptr,
            //buffer: Some(buffer),
            memory: Some(memory),
            image: Some(image),
            image_view: Some(image_view),
            sampler: Some(sampler)
        }
    }

    pub fn write_descriptor_set(
        &self,
        device: &mut B::Device,
        desc: &DescriptorSet<B>,
        binding: u32) {

        let set = desc.set.as_ref().unwrap();
        let write = vec![
            pso::DescriptorSetWrite {
                binding: binding,
                array_offset: 0,
                descriptors: Some(pso::Descriptor::CombinedImageSampler(
                    self.image_view.as_ref().unwrap(),
                    Layout::ShaderReadOnlyOptimal,
                    self.sampler.as_ref().unwrap())),
                set: set
            }
        ];

        unsafe {
            device.write_descriptor_sets(write);
        }
    }
}

impl<B: Backend> Drop for Texture<B> {
    fn drop(&mut self) {
        let device = &self.device.borrow().device;
        unsafe {
            device.destroy_sampler(self.sampler.take().unwrap());
            device.destroy_image_view(self.image_view.take().unwrap());
            device.destroy_image(self.image.take().unwrap());
            device.free_memory(self.memory.take().unwrap());
        }
    }
}


pub(super) struct DepthImage<B: Backend> {
    device: Rc<RefCell<DeviceState<B>>>,
    memory: Option<B::Memory>,
    image: Option<B::Image>,
    pub(super) image_view: Option<B::ImageView>
}

impl<B: Backend> DepthImage<B> {
    pub(super) unsafe fn new(
        device_ptr: Rc<RefCell<DeviceState<B>>>,
        adapter: &AdapterState<B>,
        width: u32,
        height: u32,
        command_pool: &mut CommandPool<B, Graphics>,
    ) -> Self {

        // find optimal depth format
        let format = device_ptr.borrow().optimal_depth_format().unwrap();
        let samples = adapter.get_max_usable_sample_count();

        let (image, memory) = create_image(
            &device_ptr.borrow().device,
            &adapter,
            Kind::D2(width as Size, height as Size, 1, samples),
            format,
            Tiling::Optimal,
            ImageUsage::DEPTH_STENCIL_ATTACHMENT,
            MemoryProperties::DEVICE_LOCAL,
            1
        );

        let image_view = {
            let device = &device_ptr.borrow().device;
            let image_view = device
                .create_image_view(
                    &image,
                    ViewKind::D2,
                    format,
                    Swizzle::NO,
                    SubresourceRange {
                        aspects: Aspects::DEPTH,
                        levels: 0..1,
                        layers: 0..1,
                    }
                )
                .unwrap();
            image_view
        };

        {
            let mut cmd_buffer = command_pool.acquire_command_buffer::<command::OneShot>();
            cmd_buffer.begin();

            let aspects = {
                if format.is_stencil() {
                    Aspects::DEPTH | Aspects::STENCIL
                } else {
                    Aspects::DEPTH
                }
            };
            let image_barrier = Barrier::Image {
                states: (Access::empty(), Layout::Undefined)
                    ..(Access::DEPTH_STENCIL_ATTACHMENT_READ | Access::DEPTH_STENCIL_ATTACHMENT_WRITE, Layout::DepthStencilAttachmentOptimal),
                target: &image,
                families: None,
                range: SubresourceRange {
                    aspects: aspects,
                    levels: 0..1,
                    layers: 0..1,
                },
            };

            cmd_buffer.pipeline_barrier(
                PipelineStage::TOP_OF_PIPE..PipelineStage::EARLY_FRAGMENT_TESTS,
                MemoryDependencies::empty(),
                &[image_barrier]
            );

            cmd_buffer.finish();
        }

        DepthImage {
            device: device_ptr,
            memory: Some(memory),
            image: Some(image),
            image_view: Some(image_view)
        }
    }
}

impl<B: Backend> Drop for DepthImage<B> {
    fn drop(&mut self) {
        let device = &self.device.borrow().device;
        unsafe {
            device.destroy_image_view(self.image_view.take().unwrap());
            device.destroy_image(self.image.take().unwrap());
            device.free_memory(self.memory.take().unwrap());
        }
    }
}


pub(super) struct ColorImage<B: Backend> {
    device: Rc<RefCell<DeviceState<B>>>,
    memory: Option<B::Memory>,
    image: Option<B::Image>,
    pub(super) image_view: Option<B::ImageView>
}

impl<B: Backend> ColorImage<B> {
    pub(super) unsafe fn new(
        device_ptr: Rc<RefCell<DeviceState<B>>>,
        adapter: &AdapterState<B>,
        swapchain: &SwapchainState<B>,
        command_pool: &mut CommandPool<B, Graphics>,
    ) -> Self {

        let width = swapchain.extent.width;
        let height = swapchain.extent.height;
        let samples = adapter.get_max_usable_sample_count();

        let (image, memory) = create_image(
            &device_ptr.borrow().device,
            &adapter,
            Kind::D2(width as Size, height as Size, 1, samples),
            swapchain.format,
            Tiling::Optimal,
            ImageUsage::TRANSIENT_ATTACHMENT | ImageUsage::COLOR_ATTACHMENT,
            MemoryProperties::DEVICE_LOCAL,
            1,
        );

        let image_view = {
            let device = &device_ptr.borrow().device;
            let image_view = device
                .create_image_view(
                    &image,
                    ViewKind::D2,
                    swapchain.format,
                    Swizzle::NO,
                    SubresourceRange {
                        aspects: Aspects::COLOR,
                        levels: 0..1,
                        layers: 0..1,
                    }
                )
                .unwrap();
            image_view
        };

        {
            let mut cmd_buffer = command_pool.acquire_command_buffer::<command::OneShot>();
            cmd_buffer.begin();

            let image_barrier = Barrier::Image {
                states: (Access::empty(), Layout::Undefined)
                    ..(Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE, Layout::ColorAttachmentOptimal),
                target: &image,
                families: None,
                range: SubresourceRange {
                    aspects: Aspects::COLOR,
                    levels: 0..1,
                    layers: 0..1,
                },
            };

            cmd_buffer.pipeline_barrier(
                PipelineStage::TOP_OF_PIPE..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                MemoryDependencies::empty(),
                &[image_barrier]
            );

            cmd_buffer.finish();
        }

        ColorImage {
            device: device_ptr,
            memory: Some(memory),
            image: Some(image),
            image_view: Some(image_view)
        }

    }
}


impl<B: Backend> Drop for ColorImage<B> {
    fn drop(&mut self) {
        let device = &self.device.borrow().device;
        unsafe {
            device.destroy_image_view(self.image_view.take().unwrap());
            device.destroy_image(self.image.take().unwrap());
            device.free_memory(self.memory.take().unwrap());
        }
    }
}
