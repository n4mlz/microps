use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    mem,
    os::{fd::AsRawFd, raw::c_char},
};

const IFNAMSIZ: usize = 16;
const TUNSETIFF: libc::c_ulong = 0x4004_54ca;
const IFF_TAP: libc::c_short = 0x0002;
const IFF_NO_PI: libc::c_short = 0x1000;

#[repr(C)]
struct IfReq {
    name: [c_char; IFNAMSIZ],
    data: [u8; 24],
}

impl IfReq {
    fn new(name: &str) -> io::Result<Self> {
        let bytes = name.as_bytes();
        if bytes.is_empty() || bytes.len() >= IFNAMSIZ {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "interface name is empty or too long",
            ));
        }
        let mut request = Self {
            name: [0; IFNAMSIZ],
            data: [0; 24],
        };
        for (destination, source) in request.name.iter_mut().zip(bytes) {
            *destination = *source as c_char;
        }
        Ok(request)
    }
}

#[derive(Debug)]
pub struct Tap {
    file: File,
}

impl Tap {
    pub fn open(name: &str) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/net/tun")?;
        let mut request = IfReq::new(name)?;
        let flags = IFF_TAP | IFF_NO_PI;
        request.data[..mem::size_of::<libc::c_short>()].copy_from_slice(&flags.to_ne_bytes());
        let result = unsafe { libc::ioctl(file.as_raw_fd(), TUNSETIFF, &mut request) };
        if result == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { file })
    }

    pub fn read_frame(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.file.read(buffer)
    }

    pub fn hardware_address(&self, name: &str) -> io::Result<[u8; 6]> {
        let socket = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
        if socket == -1 {
            return Err(io::Error::last_os_error());
        }

        let mut request = match IfReq::new(name) {
            Ok(request) => request,
            Err(error) => {
                unsafe { libc::close(socket) };
                return Err(error);
            }
        };
        let result = unsafe { libc::ioctl(socket, libc::SIOCGIFHWADDR, &mut request) };
        let ioctl_error = (result == -1).then(io::Error::last_os_error);
        let close_result = unsafe { libc::close(socket) };
        if let Some(error) = ioctl_error {
            return Err(error);
        }
        if close_result == -1 {
            return Err(io::Error::last_os_error());
        }

        let mut address = [0; 6];
        address.copy_from_slice(&request.data[2..8]);
        Ok(address)
    }

    pub fn write_frame(&mut self, frame: &[u8]) -> io::Result<usize> {
        self.file.write(frame)
    }

    pub fn configure_async(&self, signal: libc::c_int) -> io::Result<()> {
        const F_SETSIG: libc::c_int = 10;

        if unsafe { libc::fcntl(self.file.as_raw_fd(), libc::F_SETOWN, libc::getpid()) } == -1 {
            return Err(io::Error::last_os_error());
        }
        let flags = unsafe { libc::fcntl(self.file.as_raw_fd(), libc::F_GETFL) };
        if flags == -1 {
            return Err(io::Error::last_os_error());
        }
        if unsafe {
            libc::fcntl(
                self.file.as_raw_fd(),
                libc::F_SETFL,
                flags | libc::O_ASYNC | libc::O_NONBLOCK,
            )
        } == -1
        {
            return Err(io::Error::last_os_error());
        }
        if unsafe { libc::fcntl(self.file.as_raw_fd(), F_SETSIG, signal) } == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}
