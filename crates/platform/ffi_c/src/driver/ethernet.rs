use alloc::string::String;
use core::ffi::c_void;

use microps::{
    DeviceBackend, DeviceError, DeviceKey, InterfaceKey,
    protocol::{MacAddr, SocketKey},
};
use slotmap::KeyData;

use crate::{TransmitFn, os::CPlatform};

pub(crate) struct EthernetDevice {
    address: MacAddr,
    transmit: TransmitFn,
    context: *mut c_void,
}

impl EthernetDevice {
    pub(crate) fn new(address: MacAddr, transmit: TransmitFn, context: *mut c_void) -> Self {
        Self {
            address,
            transmit,
            context,
        }
    }
}

unsafe impl Send for EthernetDevice {}

impl DeviceBackend<CPlatform> for EthernetDevice {
    fn hardware_address(&self) -> Option<MacAddr> {
        Some(self.address)
    }

    fn output(
        &mut self,
        frame_type: u16,
        data: &[u8],
        destination: Option<&[u8]>,
    ) -> Result<(), DeviceError> {
        let (destination, destination_length) = destination
            .map(|value| (value.as_ptr(), value.len()))
            .unwrap_or((core::ptr::null(), 0));
        let result = unsafe {
            (self.transmit)(
                self.context,
                frame_type,
                data.as_ptr(),
                data.len(),
                destination,
                destination_length,
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(DeviceError::Backend {
                message: String::from("Ethernet transmit callback failed"),
            })
        }
    }
}

pub(crate) fn device_key(handle: u64) -> Option<DeviceKey> {
    (handle != 0).then(|| KeyData::from_ffi(handle).into())
}

pub(crate) fn interface_key(handle: u64) -> Option<InterfaceKey> {
    (handle != 0).then(|| KeyData::from_ffi(handle).into())
}

pub(crate) fn socket_key(handle: u64) -> Option<SocketKey> {
    (handle != 0).then(|| KeyData::from_ffi(handle).into())
}
