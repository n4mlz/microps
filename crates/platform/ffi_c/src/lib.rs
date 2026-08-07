#![no_std]
#![allow(clippy::missing_safety_doc)]

extern crate alloc;

mod abi;
mod api;
mod driver;
mod os;
mod socket;

pub use abi::*;
pub use api::{
    microps_device_receive, microps_ethernet_register, microps_init, microps_ipv4_default_gateway,
    microps_ipv4_register, microps_poll, microps_shutdown, microps_start, microps_tick,
};
pub use socket::{
    microps_socket_abort, microps_socket_accept, microps_socket_bind, microps_socket_close,
    microps_socket_connect, microps_socket_listen, microps_socket_open, microps_socket_recv,
    microps_socket_recv_from, microps_socket_send, microps_socket_send_to,
};

#[cfg(not(test))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
