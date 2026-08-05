use core::convert::Infallible;
use std::{
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use microps::{Platform, Stdout};

use crate::LinuxPlatform;

static TERMINATE: OnceLock<Arc<AtomicBool>> = OnceLock::new();

fn unblock_termination_signals() {
    let mut set = unsafe { core::mem::zeroed::<libc::sigset_t>() };
    let result = unsafe {
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGINT);
        libc::sigaddset(&mut set, libc::SIGTERM);
        libc::pthread_sigmask(libc::SIG_UNBLOCK, &set, core::ptr::null_mut())
    };
    assert_eq!(result, 0, "failed to unblock termination signals");
}

fn init_signal() {
    unblock_termination_signals();
    let terminate = TERMINATE
        .get_or_init(|| Arc::new(AtomicBool::new(false)))
        .clone();
    terminate.store(false, Ordering::SeqCst);
    let mut signals = signal_hook::iterator::Signals::new([
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
    ])
    .expect("failed to register termination signals");
    thread::spawn(move || {
        for _signal in signals.forever() {
            terminate.store(true, Ordering::SeqCst);
        }
    });
}

pub fn should_terminate() -> bool {
    TERMINATE
        .get()
        .is_some_and(|flag| flag.load(Ordering::SeqCst))
}

impl Platform for LinuxPlatform {
    type Error = Infallible;
    type Mutex<T: Send> = crate::os::LinuxMutex<T>;

    fn stack() -> &'static microps::Stack<Self> {
        crate::stack()
    }

    fn init() -> Result<(), <Self as Platform>::Error> {
        <LinuxPlatform as Stdout>::init();
        init_signal();
        Ok(())
    }

    fn shutdown() {}

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before UNIX epoch")
            .as_secs()
    }
}
