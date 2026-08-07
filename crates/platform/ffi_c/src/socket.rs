use core::slice;

use microps::protocol::{Socket, SocketDomain, SocketKey, SocketProtocol, SocketType};
use slotmap::Key;

use crate::{
    abi::{
        MICROPS_INVALID_ARGUMENT, MICROPS_NOT_FOUND, MICROPS_OK, MICROPS_PROTOCOL_DEFAULT,
        MICROPS_PROTOCOL_TCP, MICROPS_PROTOCOL_UDP, MICROPS_TYPE_DATAGRAM, MICROPS_TYPE_STREAM,
    },
    api::{endpoint, require_running, socket_status},
    driver::socket_key,
    os::CPlatform,
};

fn socket_open(kind: u32, protocol: u32) -> Result<SocketKey, i32> {
    let kind = match kind {
        MICROPS_TYPE_STREAM => SocketType::Stream,
        MICROPS_TYPE_DATAGRAM => SocketType::Datagram,
        _ => return Err(MICROPS_INVALID_ARGUMENT),
    };
    let protocol = match (kind, protocol) {
        (SocketType::Stream, MICROPS_PROTOCOL_DEFAULT | MICROPS_PROTOCOL_TCP) => {
            Some(SocketProtocol::Tcp)
        }
        (SocketType::Datagram, MICROPS_PROTOCOL_DEFAULT | MICROPS_PROTOCOL_UDP) => {
            Some(SocketProtocol::Udp)
        }
        _ => return Err(MICROPS_INVALID_ARGUMENT),
    };
    Socket::open::<CPlatform>(SocketDomain::Ipv4, kind, protocol).map_err(socket_status)
}

/// IPv4 の TCP または UDP socket を作成し、handle を `socket` に書き込む。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn microps_socket_open(kind: u32, protocol: u32, socket: *mut u64) -> i32 {
    if socket.is_null() {
        return MICROPS_INVALID_ARGUMENT;
    }
    if let Err(error) = require_running() {
        return error;
    }
    match socket_open(kind, protocol) {
        Ok(value) => {
            unsafe { *socket = value.data().as_ffi() };
            MICROPS_OK
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn microps_socket_bind(socket: u64, address: *const u8, port: u16) -> i32 {
    if let Err(error) = require_running() {
        return error;
    }
    let Some(socket) = socket_key(socket) else {
        return MICROPS_INVALID_ARGUMENT;
    };
    let Some(endpoint) = endpoint(address, port) else {
        return MICROPS_INVALID_ARGUMENT;
    };
    Socket::bind::<CPlatform>(socket, endpoint).map_or_else(socket_status, |_| MICROPS_OK)
}

#[unsafe(no_mangle)]
pub extern "C" fn microps_socket_listen(socket: u64, backlog: usize) -> i32 {
    if let Err(error) = require_running() {
        return error;
    }
    let Some(socket) = socket_key(socket) else {
        return MICROPS_INVALID_ARGUMENT;
    };
    Socket::listen::<CPlatform>(socket, backlog).map_or_else(socket_status, |_| MICROPS_OK)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn microps_socket_accept(
    socket: u64,
    accepted: *mut u64,
    address: *mut u8,
    port: *mut u16,
) -> i32 {
    if let Err(error) = require_running() {
        return error;
    }
    if accepted.is_null() || address.is_null() || port.is_null() {
        return MICROPS_INVALID_ARGUMENT;
    }
    let Some(socket) = socket_key(socket) else {
        return MICROPS_INVALID_ARGUMENT;
    };
    match Socket::accept::<CPlatform>(socket) {
        Ok((child, remote)) => {
            unsafe {
                *accepted = child.data().as_ffi();
                *(address.cast::<[u8; 4]>()) = *remote.address().as_bytes();
                *port = remote.port();
            }
            MICROPS_OK
        }
        Err(error) => socket_status(error),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn microps_socket_connect(socket: u64, address: *const u8, port: u16) -> i32 {
    if let Err(error) = require_running() {
        return error;
    }
    let Some(socket) = socket_key(socket) else {
        return MICROPS_INVALID_ARGUMENT;
    };
    let Some(remote) = endpoint(address, port) else {
        return MICROPS_INVALID_ARGUMENT;
    };
    Socket::connect::<CPlatform>(socket, remote).map_or_else(socket_status, |_| MICROPS_OK)
}

unsafe fn buffer<'a>(data: *const u8, length: usize) -> Option<&'a [u8]> {
    if data.is_null() && length != 0 {
        None
    } else if length == 0 {
        Some(&[])
    } else {
        Some(unsafe { slice::from_raw_parts(data, length) })
    }
}

unsafe fn buffer_mut<'a>(data: *mut u8, length: usize) -> Option<&'a mut [u8]> {
    if data.is_null() && length != 0 {
        None
    } else if length == 0 {
        Some(&mut [])
    } else {
        Some(unsafe { slice::from_raw_parts_mut(data, length) })
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn microps_socket_recv(socket: u64, data: *mut u8, length: usize) -> i64 {
    if let Err(error) = require_running() {
        return error as i64;
    }
    let Some(socket) = socket_key(socket) else {
        return MICROPS_INVALID_ARGUMENT as i64;
    };
    let Some(data) = (unsafe { buffer_mut(data, length) }) else {
        return MICROPS_INVALID_ARGUMENT as i64;
    };
    Socket::recv::<CPlatform>(socket, data)
        .map_or_else(|error| socket_status(error) as i64, |n| n as i64)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn microps_socket_send(socket: u64, data: *const u8, length: usize) -> i64 {
    if let Err(error) = require_running() {
        return error as i64;
    }
    let Some(socket) = socket_key(socket) else {
        return MICROPS_INVALID_ARGUMENT as i64;
    };
    let Some(data) = (unsafe { buffer(data, length) }) else {
        return MICROPS_INVALID_ARGUMENT as i64;
    };
    Socket::send::<CPlatform>(socket, data)
        .map_or_else(|error| socket_status(error) as i64, |n| n as i64)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn microps_socket_recv_from(
    socket: u64,
    data: *mut u8,
    length: usize,
    address: *mut u8,
    port: *mut u16,
) -> i64 {
    if let Err(error) = require_running() {
        return error as i64;
    }
    if address.is_null() || port.is_null() {
        return MICROPS_INVALID_ARGUMENT as i64;
    }
    let Some(socket) = socket_key(socket) else {
        return MICROPS_INVALID_ARGUMENT as i64;
    };
    let Some(data) = (unsafe { buffer_mut(data, length) }) else {
        return MICROPS_INVALID_ARGUMENT as i64;
    };
    match Socket::recv_from::<CPlatform>(socket, data) {
        Ok((n, remote)) => {
            unsafe {
                *(address.cast::<[u8; 4]>()) = *remote.address().as_bytes();
                *port = remote.port();
            }
            n as i64
        }
        Err(error) => socket_status(error) as i64,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn microps_socket_send_to(
    socket: u64,
    data: *const u8,
    length: usize,
    address: *const u8,
    port: u16,
) -> i64 {
    if let Err(error) = require_running() {
        return error as i64;
    }
    let Some(socket) = socket_key(socket) else {
        return MICROPS_INVALID_ARGUMENT as i64;
    };
    let Some(data) = (unsafe { buffer(data, length) }) else {
        return MICROPS_INVALID_ARGUMENT as i64;
    };
    let Some(remote) = endpoint(address, port) else {
        return MICROPS_INVALID_ARGUMENT as i64;
    };
    Socket::send_to::<CPlatform>(socket, data, remote)
        .map_or_else(|error| socket_status(error) as i64, |n| n as i64)
}

#[unsafe(no_mangle)]
pub extern "C" fn microps_socket_close(socket: u64) -> i32 {
    if let Err(error) = require_running() {
        return error;
    }
    let Some(socket) = socket_key(socket) else {
        return MICROPS_INVALID_ARGUMENT;
    };
    Socket::close::<CPlatform>(socket).map_or_else(socket_status, |_| MICROPS_OK)
}

#[unsafe(no_mangle)]
pub extern "C" fn microps_socket_abort(socket: u64) -> i32 {
    if let Err(error) = require_running() {
        return error;
    }
    let Some(socket) = socket_key(socket) else {
        return MICROPS_INVALID_ARGUMENT;
    };
    if Socket::abort::<CPlatform>(socket) {
        MICROPS_OK
    } else {
        MICROPS_NOT_FOUND
    }
}
