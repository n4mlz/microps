#![no_std]

extern crate alloc;

mod device;
pub mod driver;
pub mod interface;
mod os;
pub mod protocol;
mod stack;

pub use device::{
    Device, DeviceBackend, DeviceError, DeviceFlags, DeviceHandle, DeviceKind, DeviceMeta,
    DeviceRegistry, DeviceState,
};
pub use driver::{DummyDevice, LoopbackDevice};
pub use interface::{AddressFamily, InterfaceRegistry, NetInterface};
pub use os::{
    Irq, Lock, Platform, Random, Stdout,
    stdout::{Writer, debugdump},
};
pub use stack::{Stack, net_input};
