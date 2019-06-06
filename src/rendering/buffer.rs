use std::cell::RefCell;
use std::rc::Rc;
use gfx_hal::{Backend, CommandPool, Device, IndexType, MemoryType, Graphics};
use gfx_hal::buffer::Usage;
use gfx_hal::command;
use gfx_hal::memory::Properties;
use gfx_hal::pso;
use image;

use super::device::DeviceState;
use super::descriptors::DescriptorSet;
use super::adapter::AdapterState;

pub(super) struct BufferState<B: Backend> {
    memory: Option<B::Memory>,
    buffer: Option<B::Buffer>,
    device: Rc<RefCell<DeviceState<B>>>,
    size: u64,
}

impl <B: Backend> BufferState<B> {
    pub(super) fn get_buffer(&self) -> &B::Buffer {
        self.buffer.as_ref().unwrap()
    }

    unsafe fn new<T>(
        device_ptr: Rc<RefCell<DeviceState<B>>>,
        buffer_size: u64,
        usage: Usage,
        properties: Properties,
        memory_types: &[MemoryType],
    ) -> Self
    where T: Copy
    {
        let (memory, buffer, size) = {
            let device = &device_ptr.borrow().device;
            let mut buffer = device.create_buffer(buffer_size, usage).unwrap();
            let mem_req = device.get_buffer_requirements(&buffer);
            let size = mem_req.size;

            let memory_type = memory_types
                .iter()
                .enumerate()
                .position(|(id, mem_type)| {
                    mem_req.type_mask & (1 << id) != 0 &&
                    mem_type.properties & properties == properties
                })
                .unwrap()
                .into();

            let memory = device.allocate_memory(
                memory_type, size).unwrap();
            device.bind_buffer_memory(&memory, 0, &mut buffer).unwrap();

            (memory, buffer, size)
        };

        BufferState {
            memory: Some(memory),
            buffer: Some(buffer),
            device: device_ptr,
            size
        }
    }

    fn update_data<T>(&mut self, offset: u64, data_source: &[T]) where T: Copy {
        let device = &self.device.borrow().device;
        let stride = std::mem::size_of::<T>() as u64;
        let upload_size = data_source.len() as u64 * stride;

        assert!(offset + upload_size <= self.size);

        let memory = self.memory.as_mut().unwrap();
        unsafe {
            let mut data_target = device
                .acquire_mapping_writer::<T>(
                    &memory, offset..self.size)
                .unwrap();
            data_target[0..data_source.len()].copy_from_slice(
                data_source);
            device.release_mapping_writer(data_target).unwrap();
        }
    }
}

impl<B: Backend> Drop for BufferState<B> {
    fn drop(&mut self) {
        let device = &self.device.borrow().device;
        unsafe {
            device.destroy_buffer(self.buffer.take().unwrap());
            device.free_memory(self.memory.take().unwrap());
        }
    }
}


pub(super) struct VertexBuffer<B: Backend>(BufferState<B>);

impl <B: Backend> VertexBuffer<B> {
    pub(super) unsafe fn new<T>(
        device_ptr: Rc<RefCell<DeviceState<B>>>,
        command_pool: &mut CommandPool<B, Graphics>,
        data_source: &[T],
        memory_types: &[MemoryType],
    ) -> Self where T: Copy {
        let stride = std::mem::size_of::<T>() as u64;
        let buffer_size = data_source.len() as u64 * stride;

        let mut staging_buffer = BufferState::new::<T>(
            Rc::clone(&device_ptr),
            buffer_size,
            Usage::TRANSFER_SRC,
            Properties::CPU_VISIBLE | Properties::COHERENT,
            memory_types
        );
        staging_buffer.update_data(0, data_source);

        let vertex_buffer = BufferState::new::<T>(
            Rc::clone(&device_ptr),
            buffer_size,
            Usage::TRANSFER_DST | Usage::VERTEX,
            Properties::DEVICE_LOCAL,
            memory_types
        );

        copy_command_buffer(
            &device_ptr,
            command_pool,
            staging_buffer.get_buffer(),
            vertex_buffer.get_buffer(),
            buffer_size
        );

        VertexBuffer(vertex_buffer)
    }

    pub(super) fn get_buffer(&self) -> &B::Buffer {
        self.0.get_buffer()
    }
}

pub(super) struct IndexBuffer<B: Backend>(BufferState<B>, IndexType);

impl <B: Backend> IndexBuffer<B> {
    pub(super) unsafe fn new(
        device_ptr: Rc<RefCell<DeviceState<B>>>,
        command_pool: &mut CommandPool<B, Graphics>,
        data_source: &[u32],
        memory_types: &[MemoryType],
    ) -> Self {
        let stride = std::mem::size_of::<u32>() as u64;
        let buffer_size = data_source.len() as u64 * stride;

        let mut staging_buffer = BufferState::new::<u32>(
            Rc::clone(&device_ptr),
            buffer_size,
            Usage::TRANSFER_SRC,
            Properties::CPU_VISIBLE | Properties::COHERENT,
            memory_types
        );
        staging_buffer.update_data(0, data_source);

        let index_buffer = BufferState::new::<u32>(
            Rc::clone(&device_ptr),
            buffer_size,
            Usage::TRANSFER_DST | Usage::INDEX,
            Properties::DEVICE_LOCAL,
            memory_types
        );

        copy_command_buffer(
            &device_ptr,
            command_pool,
            staging_buffer.get_buffer(),
            index_buffer.get_buffer(),
            buffer_size
        );

        IndexBuffer(index_buffer, IndexType::U32)
    }

    pub(super) fn get_buffer(&self) -> &B::Buffer {
        self.0.get_buffer()
    }

    #[inline]
    pub(super) fn index_type(&self) -> IndexType {
        self.1
    }
}

pub(super) struct UniformBuffer<B: Backend>(BufferState<B>, DescriptorSet<B>);

impl <B: Backend> UniformBuffer<B> {
    pub(super) unsafe fn new<T>(
        device_ptr: Rc<RefCell<DeviceState<B>>>,
        memory_types: &[MemoryType],
        desc: DescriptorSet<B>,
        binding: u32
    ) -> Self where T: Copy {
        let buffer_size = std::mem::size_of::<T>() as u64;

        let buffer = BufferState::new::<T>(
            Rc::clone(&device_ptr),
            buffer_size,
            Usage::UNIFORM,
            Properties::CPU_VISIBLE | Properties::COHERENT,
            memory_types
        );

        let device = &device_ptr.borrow().device;

        let set = desc.set.as_ref().unwrap();
        let write = vec![
            pso::DescriptorSetWrite {
                binding: binding,
                array_offset: 0,
                descriptors: Some(pso::Descriptor::Buffer(
                    (&buffer).get_buffer(),
                    None..None)),
                set: set
            }
        ];

        device.write_descriptor_sets(write);

        UniformBuffer(buffer, desc)
    }

    pub fn update_data<T>(&mut self, offset: u64, data_source: &[T]) where T: Copy {
        self.0.update_data(offset, data_source);
    }

    pub(super) fn get_descriptor_set(&self) -> &B::DescriptorSet {
        self.1.set.as_ref().unwrap()
    }
}


pub(super) struct TextureBuffer<B: Backend>(BufferState<B>);

impl <B: Backend> TextureBuffer<B> {
    pub(super) unsafe fn new(
        device_ptr: Rc<RefCell<DeviceState<B>>>,
        adapter: &AdapterState<B>,
        img: &image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
        usage: Usage
    ) -> (Self, u32, u32, u32, usize) {
        let (width, height) = img.dimensions();

        let row_alignment_mask = adapter.limits.optimal_buffer_copy_pitch_alignment as u32 - 1;
        let stride = 4usize;

        let row_pitch = (width * stride as u32 + row_alignment_mask) & !row_alignment_mask;
        let upload_size = (height * row_pitch) as u64;


        let mut buffer = BufferState::new::<u8>(
            Rc::clone(&device_ptr),
            upload_size as _,
            usage,
            Properties::CPU_VISIBLE | Properties::COHERENT,
            &adapter.memory_types);

        {
            let device = &device_ptr.borrow().device;
            let memory = buffer.memory.as_mut().unwrap();
            let size = buffer.size;
            let mut data_target = device
                    .acquire_mapping_writer::<u8>(memory, 0..size)
                    .unwrap();

            for y in 0..height as usize {
                let data_source_slice = &(**img)
                    [y * (width as usize) * stride..(y + 1) * (width as usize) * stride];
                let dest_base = y * row_pitch as usize;
                data_target[dest_base..dest_base + data_source_slice.len()]
                        .copy_from_slice(data_source_slice);
            }

            device.release_mapping_writer(data_target).unwrap();
        }

        (TextureBuffer(buffer), width, height, row_pitch, stride)
    }

    pub(super) fn get_buffer(&self) -> &B::Buffer {
        self.0.get_buffer()
    }
}


unsafe fn copy_command_buffer<B>(
    device_ptr: &Rc<RefCell<DeviceState<B>>>,
    command_pool: &mut CommandPool<B, Graphics>,
    src_buffer: &B::Buffer,
    dst_buffer: &B::Buffer,
    size: u64
) where B: Backend {

    let mut cmd_buffer: command::CommandBuffer<B, gfx_hal::Graphics, command::OneShot> = {
        command_pool.acquire_command_buffer::<command::OneShot>()
    };
    cmd_buffer.begin();
    cmd_buffer.copy_buffer(src_buffer, dst_buffer, &[command::BufferCopy {
        src: 0,
        dst: 0,
        size: size
    }]);
    cmd_buffer.finish();

    let queue = &mut device_ptr.borrow_mut().queues.queues[0];
    queue.submit_nosemaphores(std::iter::once(&cmd_buffer), None);
    queue.wait_idle().unwrap();
    // explicit cmd_buffer free on Drop
}
