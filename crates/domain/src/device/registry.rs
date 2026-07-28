use alloc::vec::Vec;

use slotmap::{SlotMap, new_key_type};

use crate::{Device, DeviceError};

new_key_type! {
    /// Stable key for a device owned by a [`DeviceRegistry`].
    pub struct DeviceKey;
}

#[derive(Debug, Default)]
pub struct DeviceRegistry {
    devices: SlotMap<DeviceKey, Device>,
}

impl DeviceRegistry {
    pub fn register(&mut self, device: Device) -> DeviceKey {
        self.devices.insert(device)
    }

    pub fn device(&self, key: DeviceKey) -> Option<&Device> {
        self.devices.get(key)
    }

    pub fn device_mut(&mut self, key: DeviceKey) -> Option<&mut Device> {
        self.devices.get_mut(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = DeviceKey> + '_ {
        self.devices.keys()
    }
}

impl DeviceRegistry {
    pub fn open_all(&mut self) -> Result<(), DeviceError> {
        let keys: Vec<_> = self.devices.keys().collect();
        for (index, key) in keys.iter().copied().enumerate() {
            if let Err(error) = self.devices[key].open() {
                for key in keys[..index].iter().rev().copied() {
                    let device = &mut self.devices[key];
                    let _ = device.close();
                }
                return Err(error);
            }
        }
        Ok(())
    }

    pub fn close_all(&mut self) {
        let keys: Vec<_> = self.devices.keys().collect();
        for key in keys.into_iter().rev() {
            let _ = self.devices[key].close();
        }
    }
}
