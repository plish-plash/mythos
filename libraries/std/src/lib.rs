#![feature(core_intrinsics)]
#![feature(alloc_error_handler)]
#![no_std]
extern crate alloc;

pub mod screen;

pub use alloc::*;
pub use core::*;

use core::alloc::{GlobalAlloc, Layout};
use core::arch::asm;
use kernel_common::*;

pub type SystemError = UserError;

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

fn syscall(id: Syscall, arg_base: u64, arg_len: u64) -> Result<(u64, u64), SystemError> {
    unsafe {
        let id: u64 = mem::transmute(id);
        let ret0: u64;
        let ret1: u64;
        asm!(
            "syscall",
            in("rdi") id,
            in("rsi") arg_base,
            in("rdx") arg_len,
            lateout("rax") ret0,
            lateout("rdx") ret1,
            clobber_abi("sysv64"),
        );
        if ret0 == 0 {
            Err(mem::transmute(ret1))
        } else {
            Ok((ret0, ret1))
        }
    }
}

#[panic_handler]
fn panic(info: &panic::PanicInfo) -> ! {
    let info = format!("{}", info);
    let info = info.as_bytes();
    syscall(
        Syscall::ProgramPanic,
        info.as_ptr() as u64,
        info.len() as u64,
    )
    .unwrap_or_default();
    unreachable!();
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("alloc failed: {:?}", layout);
}

struct SystemAllocator;

#[global_allocator]
static ALLOCATOR: SystemAllocator = SystemAllocator;

unsafe impl GlobalAlloc for SystemAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        syscall(Syscall::MemAlloc, 0, layout.pack_u64()).unwrap().1 as *mut u8
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        syscall(Syscall::MemDealloc, ptr as u64, layout.pack_u64()).unwrap();
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        syscall(Syscall::MemAllocZeroed, 0, layout.pack_u64())
            .unwrap()
            .1 as *mut u8
    }
    // unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    //     syscall(Syscall::MemRealloc, ptr as u64, pack_layout(layout)).unwrap() as *mut u8
    // }
}

pub fn exit() {
    syscall(Syscall::ProgramExit, 0, 0).unwrap_or_default();
}

pub fn wait_for_confirm() {
    syscall(Syscall::ProgramWaitForConfirm, 0, 0).unwrap();
}
