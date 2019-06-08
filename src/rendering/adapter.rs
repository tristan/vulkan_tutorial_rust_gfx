use log::debug;
use gfx_hal::{Adapter, Backend, Limits, PhysicalDevice, MemoryType};

pub(super) struct AdapterState<B: Backend> {
    pub(super) adapter: Option<Adapter<B>>,
    pub(super) memory_types: Vec<MemoryType>,
    pub(super) limits: Limits
}

impl<B: Backend> AdapterState<B> {
    pub(super) fn new(adapters: &mut Vec<Adapter<B>>) -> Self {
        for adapter in adapters.iter() {
            debug!("{:?}", adapter.info);
        }

        AdapterState::<B>::new_adapter(adapters.remove(0))
    }

    pub(super) fn new_adapter(adapter: Adapter<B>) -> Self {
        let memory_types = adapter.physical_device.memory_properties().memory_types;
        let limits = adapter.physical_device.limits();
        debug!("{:?}", limits);
        debug!("{:?}", adapter.physical_device.features());

        AdapterState {
            adapter: Some(adapter),
            memory_types,
            limits
        }
    }

    pub(super) fn get_max_usable_sample_count(&self) -> u8 {
        let counts = std::cmp::min(
            self.limits.framebuffer_color_samples_count,
            self.limits.framebuffer_depth_samples_count
        );
        if counts & 64 > 0 { return 64; }
        if counts & 32 > 0 { return 32; }
        if counts & 16 > 0 { return 16; }
        if counts & 8 > 0 { return 8; }
        if counts & 4 > 0 { return 4; }
        if counts & 2 > 0 { return 2; }
        return 1;
    }
}
