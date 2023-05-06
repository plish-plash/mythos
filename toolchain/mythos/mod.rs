#![deny(unsafe_op_in_unsafe_fn)]

pub mod alloc;
pub mod args;
#[path = "../unix/cmath.rs"]
pub mod cmath;
pub mod env;
pub mod fs;
pub mod io;
pub mod locks;
pub mod net;
pub mod once;
pub mod os;
#[path = "../unix/os_str.rs"]
pub mod os_str;
#[path = "../unix/path.rs"]
pub mod path;
pub mod pipe;
pub mod process;
pub mod stdio;
pub mod thread;
#[cfg(target_thread_local)]
pub mod thread_local_dtor;
pub mod thread_local_key;
pub mod time;

mod common;
pub use common::*;

// The linker will normally include a small C-runtime file for the platform with a name like crt.o,
// which has the real entry point: the "_start" symbol. Mythos doesn't have any such file, so
// define it right here!
mod rt {
    extern "C" { fn main(argc: isize, argv: *const *const u8); }

    #[no_mangle]
    extern "C" fn _start() -> ! {
        unsafe { main(0, core::ptr::null()); }
        loop {}
    }
}
