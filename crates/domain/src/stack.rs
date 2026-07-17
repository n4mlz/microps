use crate::{
    DeviceError, DeviceHandle, DeviceMeta, DeviceRegistry, InterfaceRegistry, Platform,
    ProtocolInputQueue, debug, debugdump, error, info, warn,
};

/// Queues a received frame for deferred protocol processing.
///
/// This currently logs the frame and dispatches known EtherTypes to protocol handlers.
pub fn net_input(queue: &mut ProtocolInputQueue, meta: &DeviceMeta, frame_type: u16, data: &[u8]) {
    debug!(
        "dev={}, type=0x{frame_type:04x}, len={}",
        meta.name(),
        data.len()
    );
    debugdump(data);
    if !queue.push(frame_type, meta, data) {
        warn!("protocol input queue is full; dropping frame");
    }
}

pub fn input(
    registry: &mut DeviceRegistry,
    handle: DeviceHandle,
    queue: &mut ProtocolInputQueue,
) -> Result<(), DeviceError> {
    let device = registry
        .device_mut(handle)
        .ok_or(DeviceError::InvalidHandle { handle })?;
    let meta = device.meta().clone();
    while let Some(frame) = device.input()? {
        net_input(queue, &meta, frame.frame_type(), frame.data());
    }
    Ok(())
}

pub fn soft_input(queue: &mut ProtocolInputQueue, interfaces: &InterfaceRegistry) {
    queue.process(interfaces);
}

/// Protocol stack lifecycle.
pub struct Stack;

impl Stack {
    pub fn init<P: Platform>() -> Result<(), P::Error> {
        info!("initialize...");
        let result = P::init();
        if result.is_err() {
            error!("failure");
            return result;
        }
        info!("success");
        result
    }

    pub fn shutdown<P: Platform>() {
        info!("shutting down...");
        P::shutdown();
        info!("success");
    }
}
