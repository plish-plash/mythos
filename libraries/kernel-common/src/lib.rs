#![no_std]
extern crate alloc;

pub mod graphics;

pub struct Syscall;

impl Syscall {
    pub const INFO_OS_NAME: usize = 1;
    pub const INFO_OS_VERSION: usize = 2;
    pub const INFO_BOOTLOADER_VERSION: usize = 3;
    pub const INFO_FRAMEBUFFER: usize = 4;
    pub const INFO_GRAPHICS_CTX: usize = 5;
    pub const MEM_ALLOC: usize = 6;
    pub const MEM_DEALLOC: usize = 7;
    pub const MEM_ALLOC_ZEROED: usize = 8;
    pub const MEM_REALLOC: usize = 9;
    pub const PROGRAM_PANIC: usize = 10;

    pub const NUM_SYSCALLS: usize = 11;
}
