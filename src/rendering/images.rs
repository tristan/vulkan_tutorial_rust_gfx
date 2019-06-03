use std::cell::RefCell;
use std::rc::Rc;

use gfx_hal::{Backend, Device, CommandPool, Graphics};
use gfx_hal::buffer::Usage as BufferUsage;
use gfx_hal::image::{Access, Layout, Usage as ImageUsage,
                     Kind, Size, SubresourceLayers, Tiling,
                     ViewCapabilities, Offset, Extent};
use gfx_hal::format::{AsFormat, Aspects, Rgba8Srgb};
use gfx_hal::memory::{Barrier, Properties as MemoryProperties, Dependencies as MemoryDependencies};
use gfx_hal::command;
use gfx_hal::pso::PipelineStage;

use image;

use super::adapter::AdapterState;
use super::device::DeviceState;
use super::buffer::TextureBuffer;
use super::constants::COLOR_RANGE;

pub(super) const FOX_PNG_DATA: &'static [u8] = include_bytes!("../../textures/fox.png");

pub(super) struct Texture<B: Backend> {
    device: Rc<RefCell<DeviceState<B>>>,
    //buffer: Option<TextureBuffer<B>>,
    memory: Option<B::Memory>,
    image: Option<B::Image>
}

impl<B: Backend> Texture<B> {
    pub(super) unsafe fn new(
        device_ptr: Rc<RefCell<DeviceState<B>>>,
        adapter: &AdapterState<B>,
        command_pool: &mut CommandPool<B, Graphics>,
        img: &image::ImageBuffer<image::Rgba<u8>, Vec<u8>>
    ) -> Self {
        let (buffer, width, height, row_pitch, stride) = {
            TextureBuffer::new(
                Rc::clone(&device_ptr),
                &adapter,
                &img,
                BufferUsage::TRANSFER_SRC
            )
        };
        let (image, memory) = {
            let device = &device_ptr.borrow().device;
            let kind = Kind::D2(width as Size, height as Size, 1, 1);
            let mut image = device
                .create_image(
                    kind, // kind
                    1,  // mip_levels
                    Rgba8Srgb::SELF, // format
                    Tiling::Optimal, // tiling
                    ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED, // usage
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
                            MemoryProperties::DEVICE_LOCAL)
                })
                .unwrap()
                .into();

            let memory = device.allocate_memory(device_type, mem_req.size).unwrap();
            device.bind_image_memory(&memory, 0, &mut image).unwrap();

            // copy buffer to texture
            {
                let mut cmd_buffer = command_pool.acquire_command_buffer::<command::OneShot>();
                cmd_buffer.begin();

                let image_barrier = Barrier::Image {
                    states: (Access::empty(), Layout::Undefined)
                        ..(Access::TRANSFER_WRITE, Layout::TransferDstOptimal),
                    target: &image,
                    families: None,
                    range: COLOR_RANGE.clone(),
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
                        buffer_width: 0, // buffer_row_length
                        buffer_height: 0, // buffer_image_height
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

                // prepare image for shader access

                let image_barrier = Barrier::Image {
                    states: (Access::TRANSFER_WRITE, Layout::TransferDstOptimal)
                        ..(Access::SHADER_READ, Layout::ShaderReadOnlyOptimal),
                    target: &image,
                    families: None,
                    range: COLOR_RANGE.clone(),
                };

                cmd_buffer.pipeline_barrier(
                    PipelineStage::TRANSFER..PipelineStage::FRAGMENT_SHADER,
                    MemoryDependencies::empty(),
                    &[image_barrier],
                );

                cmd_buffer.finish();

            }
            (image, memory)
        };

        Texture {
            device: device_ptr,
            //buffer: Some(buffer),
            memory: Some(memory),
            image: Some(image)
        }
    }
}

impl<B: Backend> Drop for Texture<B> {
    fn drop(&mut self) {
        let device = &self.device.borrow().device;
        unsafe {
            device.destroy_image(self.image.take().unwrap());
            device.free_memory(self.memory.take().unwrap());
        }
    }
}