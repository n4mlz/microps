use std::{
    boxed::Box,
    collections::{HashMap, HashSet},
    sync::{Mutex, OnceLock},
};

use microps::{Irq, IrqLine};

use crate::LinuxPlatform;

type Handler = Box<dyn Fn(IrqLine) + Send + Sync>;

static HANDLERS: OnceLock<Mutex<HashMap<usize, Handler>>> = OnceLock::new();
static INSTALLED: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();

fn handlers() -> &'static Mutex<HashMap<usize, Handler>> {
    HANDLERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn installed() -> &'static Mutex<HashSet<usize>> {
    INSTALLED.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn signal_number(line: IrqLine) -> libc::c_int {
    match line {
        IrqLine::DeviceInput => libc::SIGRTMIN() + 1,
        IrqLine::SoftInput => libc::SIGUSR1,
    }
}

fn install_signal(line: IrqLine) {
    let irq = signal_number(line) as usize;
    let mut installed = installed().lock().expect("irq install mutex poisoned");
    if !installed.insert(irq) {
        return;
    }
    unsafe {
        let _ = signal_hook::low_level::register(irq as i32, move || {
            if let Some(handler) = handlers()
                .lock()
                .expect("irq registry mutex poisoned")
                .get(&irq)
            {
                handler(line);
            }
        });
    }
}

impl Irq for LinuxPlatform {
    type Error = core::convert::Infallible;

    fn register(line: IrqLine, handler: Handler) -> Result<(), Self::Error> {
        let irq = signal_number(line) as usize;
        handlers()
            .lock()
            .expect("irq registry mutex poisoned")
            .insert(irq, handler);
        install_signal(line);
        Ok(())
    }

    fn raise(line: IrqLine) -> Result<(), Self::Error> {
        let irq = signal_number(line) as usize;
        signal_hook::low_level::raise(irq as i32).expect("failed to raise signal");
        Ok(())
    }
}
