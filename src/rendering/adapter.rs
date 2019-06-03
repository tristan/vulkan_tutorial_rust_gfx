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

        AdapterState {
            adapter: Some(adapter),
            memory_types,
            limits
        }
    }
}
