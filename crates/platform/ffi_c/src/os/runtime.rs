use alloc::boxed::Box;
use core::{convert::Infallible, fmt, mem::MaybeUninit, sync::atomic::Ordering};

use microps::{Irq, IrqLine, Platform, Random, Stack, Stdout, Time};

use crate::{
    abi::{STATE, STATE_CONFIGURED, STATE_RUNNING, platform},
    os::CMutex,
};

#[derive(Clone, Copy)]
pub struct CPlatform;

impl Irq for CPlatform {
    type Error = Infallible;

    fn register(
        _line: IrqLine,
        _handler: Box<dyn Fn(IrqLine) + Send + Sync>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn raise(_line: IrqLine) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Random for CPlatform {
    type Error = Infallible;

    fn random16() -> Result<u16, Self::Error> {
        Ok(Self::random32()? as u16)
    }

    fn random32() -> Result<u32, Self::Error> {
        let ops = platform();
        Ok(unsafe { (ops.random_u32)(ops.context) })
    }
}

impl Time for CPlatform {
    fn monotonic_time_microseconds() -> u64 {
        let ops = platform();
        unsafe { (ops.time_us)(ops.context) }
    }
}

struct LogBuffer {
    bytes: [u8; 1024],
    length: usize,
}

impl LogBuffer {
    fn new() -> Self {
        Self {
            bytes: [0; 1024],
            length: 0,
        }
    }
}

impl fmt::Write for LogBuffer {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let available = self.bytes.len().saturating_sub(self.length);
        let length = available.min(value.len());
        self.bytes[self.length..self.length + length].copy_from_slice(&value.as_bytes()[..length]);
        self.length += length;
        Ok(())
    }
}

impl Stdout for CPlatform {
    fn write(args: fmt::Arguments<'_>) {
        let ops = platform();
        let mut buffer = LogBuffer::new();
        let _ = fmt::write(&mut buffer, args);
        unsafe { (ops.log)(ops.context, 0, buffer.bytes.as_ptr(), buffer.length) };
    }
}

impl Platform for CPlatform {
    type Error = Infallible;
    type Mutex<T: Send> = CMutex<T>;

    fn stack() -> &'static Stack<Self> {
        stack()
    }

    fn init() -> Result<(), <Self as Platform>::Error> {
        <Self as Stdout>::init();
        Ok(())
    }

    fn shutdown() {}
}

pub(crate) static STACK: MaybeUninit<Stack<CPlatform>> = MaybeUninit::uninit();

pub(crate) fn stack() -> &'static Stack<CPlatform> {
    assert!(matches!(
        STATE.load(Ordering::Acquire),
        STATE_CONFIGURED | STATE_RUNNING
    ));
    unsafe { STACK.assume_init_ref() }
}
