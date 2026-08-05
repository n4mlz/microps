use std::{boxed::Box, sync::Arc, thread};

use microps::{Irq, IrqLine};

use crate::LinuxPlatform;

type Handler = Box<dyn Fn(IrqLine) + Send + Sync>;

pub fn signal_number(line: IrqLine) -> libc::c_int {
    match line {
        IrqLine::DeviceInput => libc::SIGRTMIN() + 1,
        IrqLine::SoftInput => libc::SIGUSR1,
    }
}

fn run_handler(line: IrqLine, handler: Arc<dyn Fn(IrqLine) + Send + Sync>) {
    let irq = signal_number(line) as usize;
    let mut signals =
        signal_hook::iterator::Signals::new([irq as i32]).expect("failed to register IRQ signal");
    thread::spawn(move || {
        for _ in signals.forever() {
            handler(line);
        }
    });
}

impl Irq for LinuxPlatform {
    type Error = core::convert::Infallible;

    fn register(line: IrqLine, handler: Handler) -> Result<(), Self::Error> {
        run_handler(line, Arc::from(handler));
        Ok(())
    }

    fn raise(line: IrqLine) -> Result<(), Self::Error> {
        let irq = signal_number(line) as usize;
        signal_hook::low_level::raise(irq as i32).expect("failed to raise signal");
        Ok(())
    }
}
