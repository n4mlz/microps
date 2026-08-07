use alloc::slice;
use core::{ffi::c_void, sync::atomic::Ordering};

use microps::{
    DeviceKind, DeviceMeta, Platform, Stack,
    protocol::{Ipv4Addr, Ipv4Endpoint, Ipv4Interface, MacAddr, Tcp},
};
use slotmap::Key;

use crate::{
    TransmitFn,
    abi::{
        MICROPS_ALREADY_INITIALIZED, MICROPS_ERROR, MICROPS_INTERRUPTED, MICROPS_INVALID_ARGUMENT,
        MICROPS_INVALID_STATE, MICROPS_NOT_FOUND, MICROPS_NOT_INITIALIZED, MICROPS_OK,
        MicropsPlatform, PLATFORM, PLATFORM_READY, STATE, STATE_CONFIGURED, STATE_RUNNING,
        STATE_STOPPED, STATE_UNINITIALIZED,
    },
    driver::{EthernetDevice, device_key, interface_key},
    os::{CPlatform, STACK, stack},
};

pub(crate) fn endpoint(address: *const u8, port: u16) -> Option<Ipv4Endpoint> {
    if address.is_null() {
        return None;
    }
    Some(Ipv4Endpoint::new(
        Ipv4Addr::from(unsafe { *(address.cast::<[u8; 4]>()) }),
        port,
    ))
}

fn status(state: u8) -> i32 {
    match state {
        STATE_CONFIGURED | STATE_RUNNING => MICROPS_OK,
        STATE_STOPPED => MICROPS_INVALID_STATE,
        _ => MICROPS_NOT_INITIALIZED,
    }
}

pub(crate) fn require_running() -> Result<(), i32> {
    let state = STATE.load(Ordering::Acquire);
    if state == STATE_RUNNING {
        Ok(())
    } else {
        Err(status(state))
    }
}

pub(crate) fn socket_status<E>(error: microps::protocol::SocketError<E>) -> i32 {
    use microps::protocol::{
        SocketError, TcpCloseError, TcpOpenError, TcpPcbError, TcpReceiveError, TcpSendError,
        UdpPcbError,
    };
    match error {
        SocketError::NotFound => MICROPS_NOT_FOUND,
        SocketError::TcpOpen(TcpOpenError::Pcb(TcpPcbError::Interrupted))
        | SocketError::TcpReceive(TcpReceiveError::Pcb(TcpPcbError::Interrupted))
        | SocketError::TcpSend(TcpSendError::Pcb(TcpPcbError::Interrupted))
        | SocketError::TcpClose(TcpCloseError::Pcb(TcpPcbError::Interrupted))
        | SocketError::UdpPcb(UdpPcbError::Interrupted) => MICROPS_INTERRUPTED,
        SocketError::Domain | SocketError::Type | SocketError::Protocol => MICROPS_INVALID_ARGUMENT,
        _ => MICROPS_ERROR,
    }
}

/// C 側の platform 実装を設定し、Rust 側の単一 stack を初期化する。
/// allocator と mutex callback は、この関数の呼び出し前には使用されない。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn microps_init(platform: *const MicropsPlatform) -> i32 {
    if platform.is_null() {
        return MICROPS_INVALID_ARGUMENT;
    }
    if STATE.load(Ordering::Acquire) != STATE_UNINITIALIZED {
        return MICROPS_ALREADY_INITIALIZED;
    }
    let value = unsafe { *platform };
    unsafe { core::ptr::write(PLATFORM.as_ptr() as *mut MicropsPlatform, value) };
    PLATFORM_READY.store(true, Ordering::Release);
    if Stack::<CPlatform>::init().is_err() {
        return MICROPS_ERROR;
    }
    unsafe { core::ptr::write(STACK.as_ptr() as *mut Stack<CPlatform>, Stack::new()) };
    STATE.store(STATE_CONFIGURED, Ordering::Release);
    MICROPS_OK
}

/// 登録済み device を開き、network process から利用可能にする。
#[unsafe(no_mangle)]
pub extern "C" fn microps_start() -> i32 {
    if STATE.load(Ordering::Acquire) != STATE_CONFIGURED {
        return status(STATE.load(Ordering::Acquire));
    }
    if stack().open_all().is_err() {
        return MICROPS_ERROR;
    }
    STATE.store(STATE_RUNNING, Ordering::Release);
    MICROPS_OK
}

/// 待機中の socket を起こし、device を閉じる。xv6 では通常呼び出さない。
#[unsafe(no_mangle)]
pub extern "C" fn microps_shutdown() -> i32 {
    let state = STATE.load(Ordering::Acquire);
    if !matches!(state, STATE_CONFIGURED | STATE_RUNNING) {
        return status(state);
    }
    stack().interrupt_all();
    stack().close_all();
    <CPlatform as Platform>::shutdown();
    STATE.store(STATE_STOPPED, Ordering::Release);
    MICROPS_OK
}

/// Ethernet device を登録する。transmit callback の buffer は callback 中だけ有効。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn microps_ethernet_register(
    mtu: usize,
    mac: *const u8,
    transmit: TransmitFn,
    context: *mut c_void,
) -> u64 {
    if STATE.load(Ordering::Acquire) != STATE_CONFIGURED || mac.is_null() {
        return 0;
    }
    let mac = MacAddr::from(unsafe { *(mac.cast::<[u8; 6]>()) });
    let device = stack().devices.register_device(
        DeviceMeta::new("ethernet", DeviceKind::Ethernet, mtu),
        EthernetDevice::new(mac, transmit, context),
    );
    device.data().as_ffi()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn microps_ipv4_register(
    device: u64,
    address: *const u8,
    netmask: *const u8,
) -> u64 {
    if STATE.load(Ordering::Acquire) != STATE_CONFIGURED || address.is_null() || netmask.is_null() {
        return 0;
    }
    let Some(device) = device_key(device) else {
        return 0;
    };
    if !stack().devices.contains(device) {
        return 0;
    }
    let address = Ipv4Addr::from(unsafe { *(address.cast::<[u8; 4]>()) });
    let netmask = Ipv4Addr::from(unsafe { *(netmask.cast::<[u8; 4]>()) });
    let interface = stack()
        .interfaces
        .register(Ipv4Interface::new(address, netmask));
    if stack().interfaces.attach(interface, device).is_err() {
        return 0;
    }
    interface.data().as_ffi()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn microps_ipv4_default_gateway(interface: u64, gateway: *const u8) -> i32 {
    if STATE.load(Ordering::Acquire) != STATE_CONFIGURED || gateway.is_null() {
        return status(STATE.load(Ordering::Acquire));
    }
    let Some(interface) = interface_key(interface) else {
        return MICROPS_INVALID_ARGUMENT;
    };
    let gateway = Ipv4Addr::from(unsafe { *(gateway.cast::<[u8; 4]>()) });
    stack().ipv4_routes.set_default_gateway(interface, gateway);
    MICROPS_OK
}

/// process context で呼び出し、C 側 queue の frame を Rust queue へコピーする。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn microps_device_receive(
    device: u64,
    frame_type: u16,
    data: *const u8,
    length: usize,
) -> i32 {
    if let Err(error) = require_running() {
        return error;
    }
    if data.is_null() && length != 0 {
        return MICROPS_INVALID_ARGUMENT;
    }
    let Some(device) = device_key(device) else {
        return MICROPS_INVALID_ARGUMENT;
    };
    if !stack().devices.contains(device) {
        return MICROPS_NOT_FOUND;
    }
    let data = if length == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(data, length) }
    };
    stack()
        .input_queue
        .push(microps::ReceivedFrame::new(device, frame_type, data));
    MICROPS_OK
}

/// process context で Rust の受信 queue を処理する。
#[unsafe(no_mangle)]
pub extern "C" fn microps_poll() -> i32 {
    if STATE.load(Ordering::Acquire) != STATE_RUNNING {
        return status(STATE.load(Ordering::Acquire));
    }
    stack().soft_input().map_or(MICROPS_ERROR, |_| MICROPS_OK)
}

/// process context で TCP の再送と TIME_WAIT を処理する。
#[unsafe(no_mangle)]
pub extern "C" fn microps_tick() -> i32 {
    if let Err(error) = require_running() {
        return error;
    }
    Tcp::tick::<CPlatform>().map_or(MICROPS_ERROR, |_| MICROPS_OK)
}
