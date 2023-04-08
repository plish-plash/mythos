use crate::memory::{KERNEL_MEMORY, USER_MEMORY};
use core::arch::{asm, global_asm};
use kernel_common::Syscall;
use x86_64::{
    registers::segmentation::Segment,
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
    VirtAddr,
};

// Bits:
//   1: reserved (must be 1)
//   9: enable interrupts
//   12-13: allow use of port I/O
const USER_FLAGS: u64 = 0b11001000000010;

static mut TSS: TaskStateSegment = TaskStateSegment::new();
static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();

struct Segments {
    kernel_code: SegmentSelector,
    kernel_data: SegmentSelector,
    user_code: SegmentSelector,
    user_data: SegmentSelector,
    tss: SegmentSelector,
}

impl Segments {
    fn init(gdt: &mut GlobalDescriptorTable, tss: &'static TaskStateSegment) -> Segments {
        let kernel_code = gdt.add_entry(Descriptor::kernel_code_segment());
        let kernel_data = gdt.add_entry(Descriptor::kernel_data_segment());
        let tss = gdt.add_entry(Descriptor::tss_segment(tss));
        let user_data = gdt.add_entry(Descriptor::user_data_segment());
        let user_code = gdt.add_entry(Descriptor::user_code_segment());
        Segments {
            kernel_code,
            kernel_data,
            user_code,
            user_data,
            tss,
        }
    }
}

pub fn init_gdt() {
    // Setup TSS
    unsafe {
        TSS.privilege_stack_table[0] = KERNEL_MEMORY.privilege_stack.stack_start();
        TSS.interrupt_stack_table[0] = KERNEL_MEMORY.interrupt_stack.stack_start();
        TSS.interrupt_stack_table[1] = KERNEL_MEMORY.double_fault_stack.stack_start();
    }

    // Setup GDT
    unsafe {
        let segments = Segments::init(&mut GDT, &TSS);
        GDT.load();
        x86_64::registers::segmentation::CS::set_reg(segments.kernel_code);
        x86_64::registers::segmentation::SS::set_reg(segments.kernel_data);
        x86_64::instructions::tables::load_tss(segments.tss);
        setup_userspace(&segments);
    }
}

unsafe fn setup_userspace(segments: &Segments) {
    use x86_64::registers::model_specific::*;
    // Enable syscall and sysret
    Efer::update(|flags| {
        *flags |= EferFlags::SYSTEM_CALL_EXTENSIONS;
    });
    // Setup segments
    Star::write(
        segments.user_code,
        segments.user_data,
        segments.kernel_code,
        segments.kernel_data,
    )
    .unwrap();
    // Set jump point for when userspace executes syscall
    LStar::write(VirtAddr::from_ptr(syscall as *const ()));

    // Initialize function table.
    syscall_fns::init();
}

pub fn enter_userspace(entry_point: VirtAddr) -> ! {
    let user_stack: u64 = USER_MEMORY.stack.stack_start().as_u64();
    unsafe {
        asm!(
            "mov rsp, {stack}",
            "mov rbp, {stack}",
            "mov r11, {flags}",
            "sysretq",
            in("rcx") entry_point.as_u64(),
            stack = in(reg) user_stack,
            flags = const USER_FLAGS,
            options(noreturn),
        )
    }
}

#[no_mangle]
static mut _syscall_funcs: [u64; Syscall::NUM_SYSCALLS] = [0; Syscall::NUM_SYSCALLS];

#[no_mangle]
static mut _syscall_user_return: u64 = 0;

extern "C" {
    fn syscall() -> !;
}

global_asm!(
    r#"
.globl syscall
syscall:
    mov [_syscall_user_return + rip], rcx
    lea rcx, [_syscall_funcs + rip]
    add rax, rcx
    pop rcx
    call [rax]
    mov rcx, [_syscall_user_return + rip]
    mov r11, {flags}
    sysretq
"#, flags = const USER_FLAGS
);

#[allow(improper_ctypes_definitions)]
mod syscall_fns {
    use crate::{fatal_error, graphics, memory};
    use alloc::string::String;
    use core::alloc::{GlobalAlloc, Layout};
    use kernel_common::{
        graphics::{FrameBuffer, GraphicsContext},
        Syscall,
    };

    pub unsafe fn init() {
        use super::_syscall_funcs as funcs;
        funcs[Syscall::INFO_OS_NAME] = info_os_name as u64;
        funcs[Syscall::INFO_OS_VERSION] = info_os_version as u64;
        funcs[Syscall::INFO_BOOTLOADER_VERSION] = info_bootloader_version as u64;
        funcs[Syscall::INFO_FRAMEBUFFER] = info_framebuffer as u64;
        funcs[Syscall::INFO_GRAPHICS_CTX] = info_graphics_ctx as u64;
        funcs[Syscall::MEM_ALLOC] = mem_alloc as u64;
        funcs[Syscall::MEM_DEALLOC] = mem_dealloc as u64;
        funcs[Syscall::MEM_ALLOC_ZEROED] = mem_alloc_zeroed as u64;
        funcs[Syscall::MEM_REALLOC] = mem_realloc as u64;
        funcs[Syscall::PROGRAM_PANIC] = program_panic as u64;
    }

    fn copy_str_to_user_memory(input: &str) -> String {
        unsafe {
            let len = input.len();
            let buf = mem_alloc(Layout::from_size_align_unchecked(len, 1));
            core::slice::from_raw_parts_mut(buf, len).copy_from_slice(input.as_bytes());
            String::from_raw_parts(buf, len, len)
        }
    }
    extern "sysv64" fn info_os_name() -> String {
        copy_str_to_user_memory(crate::OS_NAME)
    }
    extern "sysv64" fn info_os_version() -> String {
        copy_str_to_user_memory(crate::OS_VERSION)
    }
    extern "sysv64" fn info_bootloader_version() -> String {
        let bootloader_version = unsafe { crate::BOOTLOADER_VERSION.as_deref().unwrap_or("") };
        copy_str_to_user_memory(bootloader_version)
    }
    unsafe extern "sysv64" fn info_framebuffer() -> FrameBuffer {
        graphics::framebuffer().expect("graphics not initialized")
    }
    extern "sysv64" fn info_graphics_ctx() -> GraphicsContext {
        graphics::context()
    }

    unsafe extern "sysv64" fn mem_alloc(layout: Layout) -> *mut u8 {
        memory::user_allocator().alloc(layout)
    }
    unsafe extern "sysv64" fn mem_dealloc(ptr: *mut u8, layout: Layout) {
        memory::user_allocator().dealloc(ptr, layout)
    }
    unsafe extern "sysv64" fn mem_alloc_zeroed(layout: Layout) -> *mut u8 {
        memory::user_allocator().alloc_zeroed(layout)
    }
    unsafe extern "sysv64" fn mem_realloc(
        ptr: *mut u8,
        layout: Layout,
        new_size: usize,
    ) -> *mut u8 {
        memory::user_allocator().realloc(ptr, layout, new_size)
    }

    extern "sysv64" fn program_panic(message: &str) -> ! {
        fatal_error!("userspace panic:\n{}", message);
    }
}
