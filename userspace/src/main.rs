#![feature(alloc_error_handler)]
#![feature(asm_const)]
#![no_std]
#![no_main]
extern crate alloc;

use alloc::{format, string::String};
use core::{alloc::Layout, arch::global_asm, fmt::Write};
use kernel_common::{graphics, Syscall};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut framebuffer = unsafe { syscall_info_framebuffer() };
    let context = unsafe { syscall_info_graphics_ctx() };
    graphics::load_system_font(&context, [255, 255, 255]);
    let mut writer = graphics::TextWriter::new(&context, &mut framebuffer, 0, 0);
    let os_name = unsafe { syscall_info_os_name() };
    let os_version = unsafe { syscall_info_os_version() };
    let bootloader_version = unsafe { syscall_info_bootloader_version() };
    let _ = writeln!(writer, "{} v{}", os_name, os_version);
    let _ = writeln!(writer, "Bootloader v{}", bootloader_version);

    unsafe {
        ata::init();
    }
    let drives = ata::list().unwrap();
    let _ = writeln!(writer, "{:?}", drives[0]);
    loop {}
}

#[allow(improper_ctypes)]
extern "sysv64" {
    fn syscall_info_os_name() -> String;
    fn syscall_info_os_version() -> String;
    fn syscall_info_bootloader_version() -> String;
    fn syscall_info_framebuffer() -> graphics::FrameBuffer;
    fn syscall_info_graphics_ctx() -> graphics::GraphicsContext;

    fn syscall_mem_alloc(layout: Layout) -> *mut u8;
    fn syscall_mem_dealloc(ptr: *mut u8, layout: Layout);
    fn syscall_mem_alloc_zeroed(layout: Layout) -> *mut u8;
    fn syscall_mem_realloc(ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8;

    fn syscall_program_panic(message: &str) -> !;
}

macro_rules! impl_syscall {
    ($name:expr, $id:expr) => {
        global_asm!(concat!(".globl ", $name, "\n", $name, ":\n",
            r#"
                mov rax, {syscall_addr}
                push rcx
                syscall
                ret"#),
            syscall_addr = const $id * 8);
    };
}

impl_syscall!("syscall_info_os_name", Syscall::INFO_OS_NAME);
impl_syscall!("syscall_info_os_version", Syscall::INFO_OS_VERSION);
impl_syscall!(
    "syscall_info_bootloader_version",
    Syscall::INFO_BOOTLOADER_VERSION
);
impl_syscall!("syscall_info_framebuffer", Syscall::INFO_FRAMEBUFFER);
impl_syscall!("syscall_info_graphics_ctx", Syscall::INFO_GRAPHICS_CTX);

impl_syscall!("syscall_mem_alloc", Syscall::MEM_ALLOC);
impl_syscall!("syscall_mem_dealloc", Syscall::MEM_DEALLOC);
impl_syscall!("syscall_mem_alloc_zeroed", Syscall::MEM_ALLOC_ZEROED);
impl_syscall!("syscall_mem_realloc", Syscall::MEM_REALLOC);

impl_syscall!("syscall_program_panic", Syscall::PROGRAM_PANIC);

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let info_string = format!("{}", info);
    unsafe {
        syscall_program_panic(&info_string);
    }
}

#[alloc_error_handler]
fn alloc_error_handler(_layout: Layout) -> ! {
    unsafe {
        syscall_program_panic("alloc failed");
    }
}

struct SystemAllocator;

#[global_allocator]
static ALLOCATOR: SystemAllocator = SystemAllocator;

unsafe impl core::alloc::GlobalAlloc for SystemAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        syscall_mem_alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        syscall_mem_dealloc(ptr, layout)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        syscall_mem_alloc_zeroed(layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        syscall_mem_realloc(ptr, layout, new_size)
    }
}
