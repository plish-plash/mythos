#![feature(core_intrinsics)]
#![feature(alloc_error_handler)]
#![no_std]

extern crate alloc;

pub use core::*;
pub use alloc::*;

use core::arch::asm;
use core::alloc::{GlobalAlloc, Layout};
use kernel_common::*;

fn syscall(id: Syscall, arg_base: u64, arg_len: u64) -> Result<u64, UserError> {
    unsafe {
        let id: u64 = mem::transmute(id);
        let result: u64;
        asm!(
            "syscall",
            in("rdi") id,
            in("rsi") arg_base,
            in("rdx") arg_len,
            out("rax") result,
            clobber_abi("sysv64"),
        );
        UserError::unpack(result)
    }
}

#[macro_export]
macro_rules! entry_point {
    ($path:path) => {
        #[no_mangle]
        pub extern "C" fn _start() -> ! {
            let f: fn() = $path; // validate entry point signature
            f();
            $crate::exit();
            unreachable!();
        }
    };
}

#[panic_handler]
fn panic(info: &panic::PanicInfo) -> ! {
    let info = format!("{}", info);
    let info = info.as_bytes();
    syscall(Syscall::ProgramPanic, info.as_ptr() as u64, info.len() as u64).unwrap_or_default();
    unreachable!();
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("alloc failed: {:?}", layout);
}

pub fn exit() {
    syscall(Syscall::ProgramExit, 0, 0).unwrap_or_default();
}

pub fn test_syscall() {
    syscall(Syscall::InfoOsName, 0, 0).unwrap();
}

struct SystemAllocator;

#[global_allocator]
static ALLOCATOR: SystemAllocator = SystemAllocator;

fn pack_layout(layout: Layout) -> u64 {
    ((layout.align() as u64 & u32::MAX as u64) << 32) | (layout.size() as u64 & u32::MAX as u64)
}

unsafe impl GlobalAlloc for SystemAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        syscall(Syscall::MemAlloc, 0, pack_layout(layout)).unwrap() as *mut u8
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        syscall(Syscall::MemDealloc, ptr as u64, pack_layout(layout)).unwrap();
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        syscall(Syscall::MemAllocZeroed, 0, pack_layout(layout)).unwrap() as *mut u8
    }
    // unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    //     syscall(Syscall::MemRealloc, ptr as u64, pack_layout(layout)).unwrap() as *mut u8
    // }
}
