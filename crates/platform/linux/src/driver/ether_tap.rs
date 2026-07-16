use std::{io, os::fd::RawFd};

use microps::{
    DeviceBackend, DeviceError, DeviceMeta, DeviceState, ReceivedFrame,
    protocol::{EthernetAddress, EthernetFrame, HEADER_LEN},
};

const CLONE_DEVICE: &[u8] = b"/dev/net/tun\0";
const INTERFACE_NAME_LEN: usize = 16;
const FRAME_MAX: usize = 1514;
const FRAME_MIN: usize = 60;
const F_SETSIG: libc::c_int = 10;

pub fn irq() -> usize {
    libc::SIGRTMIN() as usize + 1
}

#[repr(C)]
struct IfReq {
    name: [libc::c_char; INTERFACE_NAME_LEN],
    flags: libc::c_short,
    _padding: [u8; 22],
}

/// Linux TAP-backed Ethernet device.
#[derive(Debug)]
pub struct EtherTapDevice {
    name: String,
    address: EthernetAddress,
    fd: Option<RawFd>,
    receive_buffer: [u8; FRAME_MAX],
}

impl EtherTapDevice {
    pub fn new(name: impl Into<String>, address: EthernetAddress) -> Self {
        Self {
            name: name.into(),
            address,
            fd: None,
            receive_buffer: [0; FRAME_MAX],
        }
    }

    fn fd(&self) -> Result<RawFd, DeviceError> {
        self.fd.ok_or(DeviceError::NotOpen)
    }

    fn open_tap(&mut self) -> Result<(), DeviceError> {
        let path = CLONE_DEVICE.as_ptr().cast::<libc::c_char>();
        let fd = unsafe { libc::open(path, libc::O_RDWR | libc::O_NONBLOCK | libc::O_CLOEXEC) };
        if fd == -1 {
            return Err(io_error("open /dev/net/tun"));
        }

        let mut request = IfReq {
            name: [0; INTERFACE_NAME_LEN],
            flags: (libc::IFF_TAP | libc::IFF_NO_PI) as libc::c_short,
            _padding: [0; 22],
        };
        for (slot, byte) in request.name.iter_mut().zip(self.name.bytes()) {
            *slot = byte as libc::c_char;
        }

        let result = unsafe { libc::ioctl(fd, libc::TUNSETIFF, &mut request) };
        if result == -1 {
            let error = io_error("ioctl TUNSETIFF");
            unsafe { libc::close(fd) };
            return Err(error);
        }
        if unsafe { libc::fcntl(fd, libc::F_SETOWN, libc::getpid()) } == -1 {
            let error = io_error("fcntl F_SETOWN");
            unsafe { libc::close(fd) };
            return Err(error);
        }
        if unsafe { libc::fcntl(fd, F_SETSIG, irq() as libc::c_int) } == -1 {
            let error = io_error("fcntl F_SETSIG");
            unsafe { libc::close(fd) };
            return Err(error);
        }
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags == -1
            || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_ASYNC | libc::O_NONBLOCK) }
                == -1
        {
            let error = io_error("fcntl F_SETFL");
            unsafe { libc::close(fd) };
            return Err(error);
        }
        self.fd = Some(fd);
        Ok(())
    }

    fn close_tap(&mut self) -> Result<(), DeviceError> {
        let Some(fd) = self.fd.take() else {
            return Ok(());
        };
        if unsafe { libc::close(fd) } == -1 {
            return Err(io_error("close TAP device"));
        }
        Ok(())
    }
}

impl DeviceBackend for EtherTapDevice {
    fn open(&mut self, _meta: &DeviceMeta, _state: &DeviceState) -> Result<(), DeviceError> {
        self.open_tap()
    }

    fn close(&mut self, _meta: &DeviceMeta, _state: &DeviceState) -> Result<(), DeviceError> {
        self.close_tap()
    }

    fn output(
        &mut self,
        _meta: &DeviceMeta,
        _state: &DeviceState,
        frame_type: u16,
        data: &[u8],
        dst: Option<&[u8]>,
    ) -> Result<(), DeviceError> {
        let destination = dst.ok_or(DeviceError::MissingDestination)?;
        if destination.len() != EthernetAddress::ANY.octets().len() {
            return Err(DeviceError::InvalidDestination {
                len: destination.len(),
            });
        }
        if data.len() > FRAME_MAX - HEADER_LEN {
            return Err(DeviceError::PayloadTooLarge {
                mtu: FRAME_MAX - HEADER_LEN,
                len: data.len(),
            });
        }

        let mut destination_octets = [0; 6];
        destination_octets.copy_from_slice(destination);
        let frame = EthernetFrame::new(
            EthernetAddress::from(destination_octets),
            self.address,
            frame_type,
            data,
        );
        let mut bytes: Vec<u8> = frame.into();
        bytes.resize(bytes.len().max(FRAME_MIN), 0);

        let fd = self.fd()?;
        let written = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
        if written < 0 {
            return Err(io_error("write TAP frame"));
        }
        if written as usize != bytes.len() {
            return Err(DeviceError::Backend {
                message: format!("short TAP write: {written} of {} bytes", bytes.len()),
            });
        }
        Ok(())
    }

    fn input(
        &mut self,
        _meta: &DeviceMeta,
        _state: &DeviceState,
    ) -> Result<Option<ReceivedFrame<'_>>, DeviceError> {
        let fd = self.fd()?;
        let mut frame_buffer = [0; FRAME_MAX];
        loop {
            let length =
                unsafe { libc::read(fd, frame_buffer.as_mut_ptr().cast(), frame_buffer.len()) };
            if length < 0 {
                let error = io::Error::last_os_error();
                if matches!(
                    error.raw_os_error(),
                    Some(errno) if errno == libc::EAGAIN || errno == libc::EWOULDBLOCK
                ) {
                    return Ok(None);
                }
                if error.raw_os_error() == Some(libc::EINTR) {
                    continue;
                }
                return Err(DeviceError::Backend {
                    message: format!("read TAP frame: {error}"),
                });
            }
            if length == 0 {
                return Ok(None);
            }

            let data = &frame_buffer[..length as usize];
            let frame = EthernetFrame::try_from(data).map_err(|error| DeviceError::Backend {
                message: error.to_string(),
            })?;
            let destination = frame.destination();
            if destination != self.address && destination != EthernetAddress::BROADCAST {
                continue;
            }
            let frame_type = frame.ethertype();
            self.receive_buffer[..length as usize].copy_from_slice(data);
            let payload = &self.receive_buffer[HEADER_LEN..length as usize];
            return Ok(Some(ReceivedFrame::new(frame_type, payload)));
        }
    }
}

impl Drop for EtherTapDevice {
    fn drop(&mut self) {
        let _ = self.close_tap();
    }
}

fn io_error(operation: &str) -> DeviceError {
    DeviceError::Backend {
        message: format!("{operation}: {}", io::Error::last_os_error()),
    }
}
