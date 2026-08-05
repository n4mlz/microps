mod backend;
mod queue;
mod registry;

use alloc::{boxed::Box, string::String};

pub use backend::*;
use bitflags::bitflags;
use getset::{CopyGetters, Getters};
pub use queue::*;
pub use registry::*;
use thiserror::Error;

use crate::Platform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Dummy,
    Loopback,
    Ethernet,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct DeviceFlags: u16 {
        const UP = 0x0001;
        const LOOPBACK = 0x0010;
        const BROADCAST = 0x0020;
        const POINT_TO_POINT = 0x0040;
        const NEED_ARP = 0x0100;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Getters, CopyGetters)]
pub struct DeviceMeta {
    #[getset(get = "pub")]
    name: String,
    #[getset(get_copy = "pub")]
    kind: DeviceKind,
    #[getset(get_copy = "pub")]
    mtu: usize,
}

impl DeviceMeta {
    pub fn new(name: impl Into<String>, kind: DeviceKind, mtu: usize) -> Self {
        Self {
            name: name.into(),
            kind,
            mtu,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DeviceState {
    flags: DeviceFlags,
}

impl DeviceState {
    pub fn new() -> Self {
        Self {
            flags: DeviceFlags::empty(),
        }
    }

    pub fn is_up(&self) -> bool {
        self.flags.contains(DeviceFlags::UP)
    }

    pub fn up(&mut self) {
        self.flags.insert(DeviceFlags::UP);
    }

    pub fn down(&mut self) {
        self.flags.remove(DeviceFlags::UP);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DeviceError {
    #[error("device is already open")]
    AlreadyOpen,
    #[error("device is not open")]
    NotOpen,
    #[error("device key has not been assigned")]
    MissingDeviceKey,
    #[error("failed to raise input IRQ")]
    InputIrq,
    #[error("device backend failure: {message}")]
    Backend { message: String },
    #[error("Ethernet destination address is required")]
    MissingDestination,
    #[error("invalid Ethernet destination address length: {len} bytes")]
    InvalidDestination { len: usize },
    #[error("payload is too large: {len} bytes exceeds MTU {mtu}")]
    PayloadTooLarge { mtu: usize, len: usize },
}

#[derive(Getters)]
pub struct Device<P: Platform> {
    backend: Box<dyn DeviceBackend<P>>,
    device_key: Option<DeviceKey>,
    #[getset(get = "pub")]
    meta: DeviceMeta,
    #[getset(get = "pub")]
    state: DeviceState,
}

impl<P: Platform> Device<P> {
    pub fn new(meta: DeviceMeta, backend: impl DeviceBackend<P> + 'static) -> Self {
        Self {
            backend: Box::new(backend),
            device_key: None,
            meta,
            state: DeviceState::new(),
        }
    }
}

impl<P: Platform> Device<P> {
    pub fn open(&mut self) -> Result<(), DeviceError> {
        if self.state.is_up() {
            return Err(DeviceError::AlreadyOpen);
        }
        self.backend.open()?;
        self.state.up();
        Ok(())
    }

    pub(crate) fn set_device_key(&mut self, device: DeviceKey) {
        self.device_key = Some(device);
        self.backend.set_device_key(device);
    }

    pub fn close(&mut self) -> Result<(), DeviceError> {
        if !self.state.is_up() {
            return Err(DeviceError::NotOpen);
        }
        self.backend.close()?;
        self.state.down();
        Ok(())
    }

    pub fn output(
        &mut self,
        frame_type: u16,
        data: &[u8],
        dst: Option<&[u8]>,
    ) -> Result<(), DeviceError> {
        if !self.state.is_up() {
            return Err(DeviceError::NotOpen);
        }
        if data.len() > self.meta.mtu() {
            return Err(DeviceError::PayloadTooLarge {
                mtu: self.meta.mtu(),
                len: data.len(),
            });
        }
        self.backend.output(frame_type, data, dst)
    }

    pub fn input(&mut self) -> Result<(), DeviceError> {
        if !self.state.is_up() {
            return Err(DeviceError::NotOpen);
        }
        self.backend.input()
    }
}
