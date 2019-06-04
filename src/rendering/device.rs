use gfx_hal::{Adapter, Backend, Graphics, QueueGroup, QueueFamily,
              Capability, Surface, Gpu, PhysicalDevice, Features};

pub(super) struct DeviceState<B: Backend> {
    pub(super) device: B::Device,
    pub(super) physical_device: B::PhysicalDevice,
    pub(super) queues: QueueGroup<B, Graphics>,
}

impl<B: Backend> DeviceState<B> {
    pub(super) fn new(adapter: Adapter<B>, surface: &B::Surface) -> Self {
        // code taken from gfx_hal::adapter::Adapter::open_with
        // to manually add in features enabling
        let requested_family = adapter
            .queue_families.iter()
            .find(|family| {
                Graphics::supported_by(family.queue_type()) && surface.supports_queue_family(family) && 1 <= family.max_queues()
            });
        let priorities = vec![1.0; 1];
        let (id, families) = match requested_family {
            Some(family) => (
                family.id(),
                [(family, priorities.as_slice())]
            ),
            _ => panic!("Device initialization failed")
        };

        let Gpu { device, mut queues } =
            unsafe {
                adapter.physical_device.open(
                    &families,
                    Features::SAMPLER_ANISOTROPY
                ).unwrap()
            };

        DeviceState {
            device,
            queues: queues.take(id).unwrap(),
            physical_device: adapter.physical_device,
        }
    }
}
