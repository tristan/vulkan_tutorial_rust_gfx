use gfx_hal::{Adapter, Backend, Graphics, QueueGroup, Surface};

pub(super) struct DeviceState<B: Backend> {
    pub(super) device: B::Device,
    pub(super) physical_device: B::PhysicalDevice,
    pub(super) queues: QueueGroup<B, Graphics>,
}

impl<B: Backend> DeviceState<B> {
    pub(super) fn new(adapter: Adapter<B>, surface: &B::Surface) -> Self {
        let (device, queues) = adapter
            .open_with::<_, Graphics>(1, |family| surface.supports_queue_family(family))
            .unwrap();

        DeviceState {
            device,
            queues,
            physical_device: adapter.physical_device,
        }
    }
}
