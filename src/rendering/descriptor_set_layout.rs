use std::cell::RefCell;
use std::rc::Rc;
use gfx_hal::{Backend, Device};
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
}

impl<B: Backend> Drop for DescriptorSetLayout<B> {
    fn drop(&mut self) {
        let device = &self.device.borrow().device;
        unsafe {
            device.destroy_descriptor_set_layout(self.layout.take().unwrap());
        }
    }
}
