mod backend;
mod registry;

use alloc::boxed::Box;
use alloc::string::String;

pub use backend::{DeviceBackend, DummyDevice};
use bitflags::bitflags;
use getset::Getters;
pub use registry::{DeviceHandle, DeviceRegistry};

/// Device type used by the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    Dummy,
    Loopback,
    Ethernet,
}

bitflags! {
    /// Device state and capability bits.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct DeviceFlags: u16 {
        const UP = 0x0001;
        const LOOPBACK = 0x0010;
        const BROADCAST = 0x0020;
        const POINT_TO_POINT = 0x0040;
        const NEED_ARP = 0x0100;
    }
}

/// Immutable metadata shared by all device backends.
#[derive(Debug, Clone, PartialEq, Eq, Getters)]
pub struct DeviceMeta {
    #[getset(get = "pub")]
    name: String,
    kind: DeviceKind,
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

    pub fn kind(&self) -> DeviceKind {
        self.kind
    }

    pub fn mtu(&self) -> usize {
        self.mtu
    }
}

/// Mutable state for a registered device.
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

/// Error returned by device lifecycle and I/O operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceError {
    AlreadyOpen,
    NotOpen,
    PayloadTooLarge { mtu: usize, len: usize },
}

/// Concrete device value that owns its backend.
#[derive(Debug, Getters)]
pub struct Device {
    backend: Box<dyn DeviceBackend>,
    #[getset(get = "pub")]
    meta: DeviceMeta,
    #[getset(get = "pub")]
    state: DeviceState,
}

impl Device {
    pub fn new(meta: DeviceMeta, backend: impl DeviceBackend + 'static) -> Self {
        Self {
            backend: Box::new(backend),
            meta,
            state: DeviceState::new(),
        }
    }
}

impl Device {
    pub fn open(&mut self) -> Result<(), DeviceError> {
        if self.state.is_up() {
            return Err(DeviceError::AlreadyOpen);
        }
        self.backend.open(&self.meta, &self.state);
        self.state.up();
        Ok(())
    }

    pub fn close(&mut self) -> Result<(), DeviceError> {
        if !self.state.is_up() {
            return Err(DeviceError::NotOpen);
        }
        self.backend.close(&self.meta, &self.state);
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
        self.backend.output(&self.meta, &self.state, frame_type, data, dst);
        Ok(())
    }
}
