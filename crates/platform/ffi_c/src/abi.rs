use core::{
    ffi::c_void,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};

pub const MICROPS_OK: i32 = 0;
pub const MICROPS_ERROR: i32 = -1;
pub const MICROPS_INVALID_ARGUMENT: i32 = -2;
pub const MICROPS_NOT_FOUND: i32 = -3;
pub const MICROPS_INVALID_STATE: i32 = -4;
pub const MICROPS_INTERRUPTED: i32 = -5;
pub const MICROPS_NOT_INITIALIZED: i32 = -6;
pub const MICROPS_ALREADY_INITIALIZED: i32 = -7;

pub const MICROPS_DOMAIN_IPV4: u32 = 1;
pub const MICROPS_TYPE_DATAGRAM: u32 = 1;
pub const MICROPS_TYPE_STREAM: u32 = 2;
pub const MICROPS_PROTOCOL_DEFAULT: u32 = 0;
pub const MICROPS_PROTOCOL_TCP: u32 = 6;
pub const MICROPS_PROTOCOL_UDP: u32 = 17;

pub type AllocFn = unsafe extern "C" fn(*mut c_void, usize, usize) -> *mut c_void;
pub type DeallocFn = unsafe extern "C" fn(*mut c_void, *mut c_void, usize, usize);
pub type MutexCreateFn = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
pub type MutexDestroyFn = unsafe extern "C" fn(*mut c_void, *mut c_void);
pub type MutexFn = unsafe extern "C" fn(*mut c_void, *mut c_void);
pub type MutexWaitFn = unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32;
pub type TimeFn = unsafe extern "C" fn(*mut c_void) -> u64;
pub type RandomFn = unsafe extern "C" fn(*mut c_void) -> u32;
pub type LogFn = unsafe extern "C" fn(*mut c_void, i32, *const u8, usize);
/// 送信 callback の buffer は callback の実行中だけ有効です。
pub type TransmitFn =
    unsafe extern "C" fn(*mut c_void, u16, *const u8, usize, *const u8, usize) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
/// C 側の platform 実装。全 callback を必ず設定し、`context` を第一引数に受け取ります。
/// `mutex_wait` は mutex を保持した状態で呼ばれ、戻る時点で再取得済みです。
pub struct MicropsPlatform {
    pub context: *mut c_void,
    pub alloc: AllocFn,
    pub dealloc: DeallocFn,
    pub mutex_create: MutexCreateFn,
    pub mutex_destroy: MutexDestroyFn,
    pub mutex_lock: MutexFn,
    pub mutex_unlock: MutexFn,
    pub mutex_wait: MutexWaitFn,
    pub mutex_wake_all: MutexFn,
    pub mutex_interrupt_all: MutexFn,
    pub time_us: TimeFn,
    pub random_u32: RandomFn,
    pub log: LogFn,
}

unsafe impl Send for MicropsPlatform {}
unsafe impl Sync for MicropsPlatform {}

pub(crate) static PLATFORM: MaybeUninit<MicropsPlatform> = MaybeUninit::uninit();
pub(crate) static PLATFORM_READY: AtomicBool = AtomicBool::new(false);
pub(crate) static STATE: AtomicU8 = AtomicU8::new(0);

pub(crate) const STATE_UNINITIALIZED: u8 = 0;
pub(crate) const STATE_CONFIGURED: u8 = 1;
pub(crate) const STATE_RUNNING: u8 = 2;
pub(crate) const STATE_STOPPED: u8 = 3;

pub(crate) fn platform() -> MicropsPlatform {
    assert!(PLATFORM_READY.load(Ordering::Acquire));
    // PLATFORM は初期化後に変更されない。
    unsafe { PLATFORM.assume_init_read() }
}
