use core::{
    alloc::{GlobalAlloc, Layout},
    sync::atomic::Ordering,
};

use crate::abi::{PLATFORM_READY, platform};

struct GlobalAllocator;

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !PLATFORM_READY.load(Ordering::Acquire) {
            return core::ptr::null_mut();
        }
        let ops = platform();
        unsafe { (ops.alloc)(ops.context, layout.size(), layout.align()) as *mut u8 }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() || !PLATFORM_READY.load(Ordering::Acquire) {
            return;
        }
        let ops = platform();
        unsafe { (ops.dealloc)(ops.context, ptr.cast(), layout.size(), layout.align()) }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator;
