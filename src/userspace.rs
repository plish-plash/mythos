use core::arch::asm;
use x86_64::{
    VirtAddr,
    structures::{gdt::{GlobalDescriptorTable, Descriptor, SegmentSelector}, tss::TaskStateSegment},
    registers::segmentation::Segment,
};
use kernel_common::*;
use crate::{memory, program};

static mut TSS: TaskStateSegment = TaskStateSegment::new();
static mut GDT: GlobalDescriptorTable = GlobalDescriptorTable::new();

const STACK_SIZE: usize = 4096 * 2;
static mut INTERRUPT_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
static mut DOUBLE_FAULT_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

struct Segments {
    kernel_code: SegmentSelector,
    kernel_data: SegmentSelector,
    user_code: SegmentSelector,
    user_data: SegmentSelector,
    tss: SegmentSelector,
}

impl Segments {
    fn init(gdt: &mut GlobalDescriptorTable) -> Segments {
        let kernel_code = gdt.add_entry(Descriptor::kernel_code_segment());
        let kernel_data = gdt.add_entry(Descriptor::kernel_data_segment());
        let tss = gdt.add_entry(Descriptor::tss_segment(unsafe { &TSS }));
        let user_data = gdt.add_entry(Descriptor::user_data_segment());
        let user_code = gdt.add_entry(Descriptor::user_code_segment());
        Segments { kernel_code, kernel_data, user_code, user_data, tss }
    }
}

pub fn init_gdt() {
    // Setup TSS
    unsafe {
        TSS.privilege_stack_table[0] = memory::KERNEL_STACK_MEMORY.stack_start();
        TSS.interrupt_stack_table[0] = VirtAddr::from_ptr(INTERRUPT_STACK.as_ptr_range().end.offset(-16));
        TSS.interrupt_stack_table[1] = VirtAddr::from_ptr(DOUBLE_FAULT_STACK.as_ptr_range().end.offset(-16));
    }

    // Setup GDT
    unsafe {
        let segments = Segments::init(&mut GDT);
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
        segments.user_code, segments.user_data,
        segments.kernel_code, segments.kernel_data).unwrap();
    // Set jump point for when userspace executes syscall
    LStar::write(VirtAddr::from_ptr(syscall as *const ()));
}

pub fn enter_userspace(entry_point: VirtAddr) -> ! {
    let user_stack: u64 = memory::USER_STACK_MEMORY.stack_start().as_u64();
    unsafe {
        asm!(
            "mov rsp, {stack}",
            "mov r11, 0x202",
	        "sysretq",
            stack = in(reg) user_stack,
            in("rcx") entry_point.as_u64(),
            options(noreturn),
        )
    }
}

pub fn restore_userspace(user_stack: u64) -> ! {
    unsafe {
        asm!(
            "mov rsp, {stack}",
            "pop rcx",
            "mov r11, 0x202",
            "sysretq",
            stack = in(reg) user_stack,
            options(noreturn),
        )
    }
}

fn unpack_layout(arg: u64) -> core::alloc::Layout {
    core::alloc::Layout::from_size_align(
        (arg & u32::MAX as u64) as usize,
        ((arg >> 32) & u32::MAX as u64) as usize,
    ).unwrap()
}

#[no_mangle]
extern "sysv64" fn _syscall_handler(id: Syscall, arg_base: u64, arg_len: u64, user_stack: u64) -> u64 {
    let mut result = Ok(0);
    match id {
        Syscall::InfoOsName => {
            // TODO
            log::info!("Hello from userspace!");
        },
        Syscall::InfoOsVersion => unimplemented!(),
        Syscall::MemAlloc => program::with_current_program_allocator(|alloc| {
            let layout = unpack_layout(arg_len);
            result = unsafe { Ok(alloc.alloc(layout) as u64) };
        }),
        Syscall::MemDealloc => program::with_current_program_allocator(|alloc| {
            let layout = unpack_layout(arg_len);
            unsafe { alloc.dealloc(arg_base as *mut u8, layout); }
        }),
        Syscall::MemAllocZeroed => program::with_current_program_allocator(|alloc| {
            let layout = unpack_layout(arg_len);
            result = unsafe { Ok(alloc.alloc_zeroed(layout) as u64) };
        }),
        Syscall::MemRealloc => unimplemented!(),
        Syscall::ProgramExit => program::current_program_exit(),
        Syscall::ProgramPanic => {
            let info = unsafe { core::slice::from_raw_parts(arg_base as *const u8, arg_len as usize) };
            let info = core::str::from_utf8(info).unwrap();
            log::warn!("Program aborted: {}", info);
            crate::logger::show_kernel_screen();
            program::current_program_exit();
        }
        Syscall::ProgramLoad => unimplemented!(),
        Syscall::ScreenCreate => unimplemented!(),
    }
    UserError::pack(result)
}

#[naked]
unsafe extern "sysv64" fn syscall() -> ! {
    asm!(
        "push rcx",
        "call _syscall_handler",
        "pop rcx",
        "mov r11, 0x202",
        "sysretq",
        options(noreturn)
    )
}
