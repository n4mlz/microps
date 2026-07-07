use core::{
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
};

mod log;

/// `fmt::Write` adapter used by the printing macros.
pub struct Writer;

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write(format_args!("{s}"));
        Ok(())
    }
}

/// Platform-provided standard output sink.
pub trait Stdout {
    fn init() {
        set_writer(Self::write);
    }

    fn write(args: fmt::Arguments<'_>);
}

type WriteFn = for<'a> fn(fmt::Arguments<'a>);

static WRITER: AtomicUsize = AtomicUsize::new(0);

fn set_writer(write: WriteFn) {
    WRITER.store(write as usize, Ordering::Release);
}

pub fn write(args: fmt::Arguments<'_>) {
    let ptr = WRITER.load(Ordering::Acquire);
    if ptr != 0 {
        let write = unsafe { core::mem::transmute::<usize, WriteFn>(ptr) };
        write(args);
    }
}

pub use log::debugdump;
