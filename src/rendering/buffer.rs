use std::cell::RefCell;
use std::rc::Rc;
use gfx_hal::{Backend, Device, IndexType, MemoryType};
use gfx_hal::buffer::Usage;
use gfx_hal::command;
use gfx_hal::memory::Properties;
use gfx_hal::pool;
use super::device::DeviceState;

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

    unsafe fn update_data<T>(&mut self, data_source: &[T]) where T: Copy {
        let device = &self.device.borrow().device;
        let memory = self.memory.as_mut().unwrap();
        let mut data_target = device
            .acquire_mapping_writer::<T>(&memory, 0..self.size)
            .unwrap();
        data_target[0..data_source.len()].copy_from_slice(data_source);
        device.release_mapping_writer(data_target).unwrap();
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
        staging_buffer.update_data(data_source);

        let vertex_buffer = BufferState::new::<T>(
            Rc::clone(&device_ptr),
            buffer_size,
            Usage::TRANSFER_DST | Usage::VERTEX,
            Properties::DEVICE_LOCAL,
            memory_types
        );

        copy_command_buffer(
            &device_ptr,
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
    pub(super) unsafe fn new<T>(
        device_ptr: Rc<RefCell<DeviceState<B>>>,
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
        staging_buffer.update_data(data_source);

        let index_buffer = BufferState::new::<T>(
            Rc::clone(&device_ptr),
            buffer_size,
            Usage::TRANSFER_DST | Usage::INDEX,
            Properties::DEVICE_LOCAL,
            memory_types
        );

        copy_command_buffer(
            &device_ptr,
            staging_buffer.get_buffer(),
            index_buffer.get_buffer(),
            buffer_size
        );

        IndexBuffer(index_buffer, IndexType::U16)
    }

    pub(super) fn get_buffer(&self) -> &B::Buffer {
        self.0.get_buffer()
    }

    #[inline]
    pub(super) fn index_type(&self) -> IndexType {
        self.1
    }
}

pub(super) struct UniformBuffer<B: Backend>(BufferState<B>);

impl <B: Backend> UniformBuffer<B> {
    pub(super) unsafe fn new<T>(
        device_ptr: Rc<RefCell<DeviceState<B>>>,
        memory_types: &[MemoryType],
    ) -> Self where T: Copy {
        let buffer_size = std::mem::size_of::<T>() as u64;

        let uniform_buffer = BufferState::new::<T>(
            Rc::clone(&device_ptr),
            buffer_size,
            Usage::UNIFORM,
            Properties::CPU_VISIBLE | Properties::COHERENT,
            memory_types
        );

        UniformBuffer(uniform_buffer)
    }

    pub unsafe fn update_data<T>(&mut self, data_source: &[T]) where T: Copy {
        let device = &self.0.device.borrow().device;
        let memory = self.0.memory.as_mut().unwrap();
        let mut data_target = device
            .acquire_mapping_writer::<T>(&memory, 0..self.0.size)
            .unwrap();
        data_target[0..data_source.len()].copy_from_slice(data_source);
        device.release_mapping_writer(data_target).unwrap();
    }
}


unsafe fn copy_command_buffer<B>(
    device_ptr: &Rc<RefCell<DeviceState<B>>>,
    src_buffer: &B::Buffer,
    dst_buffer: &B::Buffer,
    size: u64
) where B: Backend {
    let mut command_pool = device_ptr
        .borrow()
        .device
        .create_command_pool_typed(
            &device_ptr.borrow().queues,
            pool::CommandPoolCreateFlags::TRANSIENT,
        )
        .expect("Can't create command pool");

    {
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

    device_ptr.borrow().device.destroy_command_pool(
        command_pool.into_raw());
}
