use std::{
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use linux::LinuxPlatform;
use microps::{Stack, Stdout};

static TERMINATE: OnceLock<Arc<AtomicBool>> = OnceLock::new();

fn init_signal() {
    let terminate = TERMINATE
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone();
    terminate.store(false, Ordering::SeqCst);
    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, terminate);
}

fn should_terminate() -> bool {
    TERMINATE
        .get()
        .is_some_and(|flag| flag.load(Ordering::SeqCst))
}

fn main() {
    <LinuxPlatform as Stdout>::init();
    init_signal();

    Stack::init::<LinuxPlatform>().unwrap();

    while !should_terminate() {
        Stack::poll::<LinuxPlatform>().unwrap();
        thread::sleep(Duration::from_millis(1));
    }

    Stack::shutdown::<LinuxPlatform>();
}
