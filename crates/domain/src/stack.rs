use crate::{DeviceMeta, InterfaceRegistry, Platform, debug, debugdump, error, info, protocol};

/// Delivers a received frame to the protocol stack.
///
/// This currently logs the frame and dispatches known EtherTypes to protocol handlers.
pub fn net_input(meta: &DeviceMeta, interfaces: &InterfaceRegistry, frame_type: u16, data: &[u8]) {
    debug!(
        "dev={}, type=0x{frame_type:04x}, len={}",
        meta.name(),
        data.len()
    );
    debugdump(data);
    protocol::Protocols::input(frame_type, meta, data, interfaces)
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
