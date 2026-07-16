use crate::{DeviceError, DeviceMeta, DeviceState, ReceivedFrame};

/// Backend hooks for platform-specific device behavior.
pub trait DeviceBackend: core::fmt::Debug + Send {
    fn open(&mut self, _meta: &DeviceMeta, _state: &DeviceState) -> Result<(), DeviceError> {
        Ok(())
    }

    fn close(&mut self, _meta: &DeviceMeta, _state: &DeviceState) -> Result<(), DeviceError> {
        Ok(())
    }

    fn output(
        &mut self,
        meta: &DeviceMeta,
        state: &DeviceState,
        frame_type: u16,
        data: &[u8],
        dst: Option<&[u8]>,
    ) -> Result<(), DeviceError>;

    fn input(
        &mut self,
        _meta: &DeviceMeta,
        _state: &DeviceState,
    ) -> Result<Option<ReceivedFrame<'_>>, DeviceError> {
        Ok(None)
    }
}
