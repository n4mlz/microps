use alloc::vec::Vec;

use crate::{Device, DeviceError};

/// Handle for a device stored in a registry.
pub type DeviceHandle = usize;

/// Ordered device registry.
#[derive(Debug, Default)]
pub struct DeviceRegistry {
    devices: Vec<Device>,
}

impl DeviceRegistry {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    pub fn register(&mut self, device: Device) -> DeviceHandle {
        let index = self.devices.len();
        self.devices.push(device);
        index
    }

    pub fn device(&self, handle: DeviceHandle) -> Option<&Device> {
        self.devices.get(handle)
    }

    pub fn device_mut(&mut self, handle: DeviceHandle) -> Option<&mut Device> {
        self.devices.get_mut(handle)
    }
}

impl DeviceRegistry {
    pub fn open_all(&mut self) -> Result<(), DeviceError> {
        for index in 0..self.devices.len() {
            if let Err(error) = self.devices[index].open() {
                for device in self.devices[..index].iter_mut().rev() {
                    let _ = device.close();
                }
                return Err(error);
            }
        }
        Ok(())
    }

    pub fn close_all(&mut self) {
        for device in self.devices.iter_mut().rev() {
            let _ = device.close();
        }
    }
}
