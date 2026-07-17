use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    thread::{self, JoinHandle},
};

use microps::Irq;
use signal_hook::{
    consts::{SIGHUP, SIGINT},
    iterator::Signals,
};

use crate::LinuxPlatform;

type Handler = Box<dyn Fn() + Send>;

static HANDLERS: OnceLock<Mutex<HashMap<usize, Handler>>> = OnceLock::new();
static THREAD: OnceLock<Mutex<Option<JoinHandle<()>>>> = OnceLock::new();

fn handlers() -> &'static Mutex<HashMap<usize, Handler>> {
    HANDLERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn thread() -> &'static Mutex<Option<JoinHandle<()>>> {
    THREAD.get_or_init(|| Mutex::new(None))
}

impl Irq for LinuxPlatform {
    type Error = String;

    fn register(irq: usize, handler: Handler) -> Result<(), Self::Error> {
        handlers()
            .lock()
            .map_err(|_| "IRQ registry mutex poisoned".to_owned())?
            .insert(irq, handler);
        Ok(())
    }

    fn raise(irq: usize) -> Result<(), Self::Error> {
        signal_hook::low_level::raise(irq as i32).map_err(|error| error.to_string())
    }

    fn run() -> Result<(), Self::Error> {
        let mut slot = thread()
            .lock()
            .map_err(|_| "IRQ thread mutex poisoned".to_owned())?;
        if slot.is_some() {
            return Ok(());
        }

        let mut signals = Signals::new([
            SIGHUP,
            SIGINT,
            crate::driver::ether_tap_irq() as i32,
            crate::driver::SOFT_IRQ as i32,
        ])
        .map_err(|error| error.to_string())?;
        *slot = Some(thread::spawn(move || {
            for signal in signals.forever() {
                if signal == SIGHUP {
                    break;
                }
                let registered = handlers().lock().expect("IRQ registry mutex poisoned");
                if let Some(handler) = registered.get(&(signal as usize)) {
                    handler();
                }
            }
        }));
        Ok(())
    }

    fn shutdown() {
        let Some(handle) = thread().lock().expect("IRQ thread mutex poisoned").take() else {
            return;
        };
        let _ = signal_hook::low_level::raise(SIGHUP);
        let _ = handle.join();
        handlers()
            .lock()
            .expect("IRQ registry mutex poisoned")
            .clear();
    }
}
