use core::convert::Infallible;
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};

use microps::{Irq, Platform, Stdout};

use crate::LinuxPlatform;

static TERMINATE: OnceLock<Arc<AtomicBool>> = OnceLock::new();

fn init_signal() {
    let terminate = TERMINATE
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone();
    terminate.store(false, Ordering::SeqCst);
    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, terminate);
}

pub fn should_terminate() -> bool {
    TERMINATE
        .get()
        .is_some_and(|flag| flag.load(Ordering::SeqCst))
}

impl Platform for LinuxPlatform {
    type Error = Infallible;

    fn init() -> Result<(), Self::Error> {
        <LinuxPlatform as Stdout>::init();
        init_signal();
        Ok(())
    }

    fn shutdown() {
        <LinuxPlatform as Irq>::shutdown();
    }
}
