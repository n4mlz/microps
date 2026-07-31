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

    pub fn write_frame(&mut self, frame: &[u8]) -> io::Result<usize> {
        self.file.write(frame)
    }
}
