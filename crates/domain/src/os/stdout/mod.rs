use core::{
    fmt,
    sync::atomic::{AtomicUsize, Ordering},
};

mod log;

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

pub(crate) fn write(args: fmt::Arguments<'_>) {
    let ptr = WRITER.load(Ordering::Acquire);
    if ptr != 0 {
        let write = unsafe { core::mem::transmute::<usize, WriteFn>(ptr) };
        write(args);
    }
}
