use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, OnceLock},
};

use microps::Irq;

use crate::LinuxPlatform;

type Handler = fn(usize, usize);

static HANDLERS: OnceLock<Mutex<HashMap<usize, (Handler, usize)>>> = OnceLock::new();
static INSTALLED: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();

fn handlers() -> &'static Mutex<HashMap<usize, (Handler, usize)>> {
    HANDLERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn installed() -> &'static Mutex<HashSet<usize>> {
    INSTALLED.get_or_init(|| Mutex::new(HashSet::new()))
}

fn install_signal(irq: usize) {
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
                handler(irq, arg);
            }
        });
    }
}

impl Irq for LinuxPlatform {
    type Error = core::convert::Infallible;

    fn register(irq: usize, handler: Handler, arg: usize) -> Result<(), Self::Error> {
        handlers()
            .lock()
            .expect("irq registry mutex poisoned")
            .insert(irq, (handler, arg));
        install_signal(irq);
        Ok(())
    }

    fn raise(irq: usize) -> Result<(), Self::Error> {
        signal_hook::low_level::raise(irq as i32).expect("failed to raise signal");
        Ok(())
    }
}
