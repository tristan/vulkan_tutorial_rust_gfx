use std::cell::RefCell;
use std::rc::Rc;
use gfx_hal::{Backend, Device, DescriptorPool};
use gfx_hal::pso;
use super::device::DeviceState;

pub(super) struct DescriptorSetLayout<B: Backend> {
    layout: Option<B::DescriptorSetLayout>,
    device: Rc<RefCell<DeviceState<B>>>,
}

impl<B: Backend> DescriptorSetLayout<B> {
    pub(super) unsafe fn new(
        device: Rc<RefCell<DeviceState<B>>>,
        bindings: Vec<pso::DescriptorSetLayoutBinding>,
    ) -> Self {
        let desc_set_layout = device
            .borrow()
            .device
            .create_descriptor_set_layout(bindings, &[])
            .ok();

        DescriptorSetLayout {
            layout: desc_set_layout,
            device,
        }
    }

    pub(super) fn get_layout(&self) -> &B::DescriptorSetLayout {
        self.layout.as_ref().unwrap()
    }

    pub(super) unsafe fn create_desc_sets(&self, desc_pool: &mut B::DescriptorPool, size: usize) -> Vec<DescriptorSet<B>> {
        let mut results = Vec::with_capacity(size);
        let layout = self.layout.as_ref().unwrap();
        let layouts = std::iter::repeat(layout).take(size);
        //let layouts: Vec<_> = (0..size).map(|_| layout.clone()).collect();
        desc_pool
            .allocate_sets(layouts, &mut results)
            .unwrap();
        results.into_iter().map(|desc_set| {
            DescriptorSet {
                set: Some(desc_set),
            }
        }).collect()
    }

}

impl<B: Backend> Drop for DescriptorSetLayout<B> {
    fn drop(&mut self) {
        let device = &self.device.borrow().device;
        unsafe {
            device.destroy_descriptor_set_layout(self.layout.take().unwrap());
        }
    }
}

pub(super) struct DescriptorSet<B: Backend> {
    pub(super) set: Option<B::DescriptorSet>
}
