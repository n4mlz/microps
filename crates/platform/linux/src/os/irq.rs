use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, OnceLock},
};

use microps::{Irq, IrqLine};

use crate::LinuxPlatform;

type Handler = fn(IrqLine, usize);

static HANDLERS: OnceLock<Mutex<HashMap<usize, (Handler, usize)>>> = OnceLock::new();
static INSTALLED: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();

fn handlers() -> &'static Mutex<HashMap<usize, (Handler, usize)>> {
    HANDLERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn installed() -> &'static Mutex<HashSet<usize>> {
    INSTALLED.get_or_init(|| Mutex::new(HashSet::new()))
}

fn signal(line: IrqLine) -> usize {
    match line {
        IrqLine::SoftInput => libc::SIGUSR1 as usize,
    }
}

fn install_signal(line: IrqLine) {
    let irq = signal(line);
    let mut installed = installed().lock().expect("irq install mutex poisoned");
    if !installed.insert(irq) {
        return;
    }
    unsafe {
        let _ = signal_hook::low_level::register(irq as i32, move || {
            if let Some((handler, arg)) = handlers()
                .lock()
                .expect("irq registry mutex poisoned")
                .get(&irq)
                .copied()
            {
                handler(line, arg);
            }
        });
    }
}

impl Irq for LinuxPlatform {
    type Error = core::convert::Infallible;

    fn register(line: IrqLine, handler: Handler, arg: usize) -> Result<(), Self::Error> {
        let irq = signal(line);
        handlers()
            .lock()
            .expect("irq registry mutex poisoned")
            .insert(irq, (handler, arg));
        install_signal(line);
        Ok(())
    }

    fn raise(line: IrqLine) -> Result<(), Self::Error> {
        let irq = signal(line);
        signal_hook::low_level::raise(irq as i32).expect("failed to raise signal");
        Ok(())
    }
}
