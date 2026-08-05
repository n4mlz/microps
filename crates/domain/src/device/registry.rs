use alloc::vec::Vec;

use slotmap::{SlotMap, new_key_type};

use crate::{Device, DeviceError, DeviceKind, Lock, Platform};

new_key_type! {
    /// Stable key for a device owned by a [`DeviceRegistry`].
    pub struct DeviceKey;
}

pub struct DeviceRegistry<P: Platform> {
    devices: P::Mutex<SlotMap<DeviceKey, Device<P>>>,
}

type Devices<P> = SlotMap<DeviceKey, Device<P>>;
type DeviceGuard<'a, P> = <<P as Platform>::Mutex<Devices<P>> as Lock<Devices<P>>>::Guard<'a>;
type DeviceLockError<P> = <<P as Platform>::Mutex<Devices<P>> as Lock<Devices<P>>>::Error;

impl<P: Platform> Default for DeviceRegistry<P> {
    fn default() -> Self {
        Self {
            devices: P::Mutex::new(SlotMap::default()),
        }
    }
}

impl<P: Platform> DeviceRegistry<P> {
    pub fn register(&self, device: Device<P>) -> DeviceKey {
        let mut devices = self
            .devices
            .acquire()
            .expect("device registry lock is infallible");
        let key = devices.insert(device);
        devices[key].set_device_key(key);
        key
    }

    pub fn acquire(&self) -> Result<DeviceGuard<'_, P>, DeviceLockError<P>> {
        self.devices.acquire()
    }

    pub fn contains(&self, key: DeviceKey) -> bool {
        self.devices
            .acquire()
            .expect("device registry lock is infallible")
            .contains_key(key)
    }

    pub fn hardware_address(&self, key: DeviceKey) -> Option<crate::protocol::MacAddr> {
        self.devices
            .acquire()
            .expect("device registry lock is infallible")
            .get(key)
            .and_then(Device::hardware_address)
    }

    pub fn keys(&self) -> Vec<DeviceKey> {
        self.devices
            .acquire()
            .expect("device registry lock is infallible")
            .keys()
            .collect()
    }

    pub fn output(
        &self,
        key: DeviceKey,
        frame_type: u16,
        data: &[u8],
        destination: Option<&[u8]>,
    ) -> Result<(), DeviceError> {
        let loopback = {
            let mut devices = self
                .devices
                .acquire()
                .expect("device registry lock is infallible");
            let device = devices.get_mut(key).ok_or(DeviceError::NotOpen)?;
            let loopback = device.meta().kind() == DeviceKind::Loopback;
            device.output(frame_type, data, destination)?;
            loopback
        };

        if loopback {
            P::raise(crate::IrqLine::SoftInput).map_err(|_| DeviceError::InputIrq)?;
        }
        Ok(())
    }

    pub fn open_all(&self) -> Result<(), DeviceError> {
        let mut devices = self
            .devices
            .acquire()
            .expect("device registry lock is infallible");
        let keys: Vec<_> = devices.keys().collect();
        for (index, key) in keys.iter().copied().enumerate() {
            if let Err(error) = devices[key].open() {
                for key in keys[..index].iter().rev().copied() {
                    let device = &mut devices[key];
                    let _ = device.close();
                }
                return Err(error);
            }
        }
        Ok(())
    }

    pub fn close_all(&self) {
        let mut devices = self
            .devices
            .acquire()
            .expect("device registry lock is infallible");
        let keys: Vec<_> = devices.keys().collect();
        for key in keys.into_iter().rev() {
            let _ = devices[key].close();
        }
    }
}
