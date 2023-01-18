use crate::memory;
use core::arch::asm;
use kernel_common::*;
use uniquelock::UniqueOnce;
use x86_64::{
    registers::segmentation::Segment,
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
    VirtAddr,
};

static TSS: UniqueOnce<TaskStateSegment> = UniqueOnce::new();
static GDT: UniqueOnce<GlobalDescriptorTable> = UniqueOnce::new();

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
        let tss = gdt.add_entry(Descriptor::tss_segment(TSS.get().unwrap()));
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
    TSS.call_once(|| {
        let mut tss = TaskStateSegment::new();
        tss.privilege_stack_table[0] = memory::KERNEL_STACK_MEMORY.stack_start();
        tss.interrupt_stack_table[0] =
            unsafe { VirtAddr::from_ptr(INTERRUPT_STACK.as_ptr_range().end.offset(-16)) };
        tss.interrupt_stack_table[1] =
            unsafe { VirtAddr::from_ptr(DOUBLE_FAULT_STACK.as_ptr_range().end.offset(-16)) };
        tss
    })
    .expect("init_gdt called twice");

    // Setup GDT
    let mut gdt = GlobalDescriptorTable::new();
    let segments = Segments::init(&mut gdt);
    GDT.call_once(|| gdt).unwrap();
    GDT.get().unwrap().load();
    unsafe {
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

#[no_mangle]
extern "sysv64" fn _syscall_handler(
    id: Syscall,
    arg_base: u64,
    arg_len: u64,
    user_stack: u64,
) -> SyscallRetValue {
    // match id {
    //     Syscall::InfoOsName => syscall_info_os_name(arg_base, arg_len).into(),
    //     Syscall::InfoOsVersion => syscall_info_os_version(arg_base, arg_len).into(),
    //     Syscall::MemAlloc => syscall_mem_alloc(arg_base, arg_len).into(),
    //     Syscall::MemDealloc => syscall_mem_dealloc(arg_base, arg_len).into(),
    //     Syscall::MemAllocZeroed => syscall_mem_alloc_zeroed(arg_base, arg_len).into(),
    //     Syscall::MemRealloc => syscall_mem_realloc(arg_base, arg_len).into(),
    //     Syscall::ProgramExit => syscall_program_exit(arg_base, arg_len),
    //     Syscall::ProgramPanic => syscall_program_panic(arg_base, arg_len),
    //     Syscall::ProgramLoad => syscall_program_load(arg_base, arg_len, user_stack),
    //     Syscall::ProgramWaitForConfirm => {
    //         syscall_program_wait_for_confirm(arg_base, arg_len).into()
    //     }
    //     Syscall::ScreenCreate => syscall_screen_create(arg_base, arg_len).into(),
    //     Syscall::ScreenSetChar => syscall_screen_set_char(arg_base, arg_len).into(),
    //     Syscall::ScreenSetPixel => syscall_screen_set_pixel(arg_base, arg_len).into(),
    // }
    unimplemented!("syscall");
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

// fn syscall_info_os_name(_arg_base: u64, _arg_len: u64) -> Result<(), UserError> {
//     // TODO
//     log::info!("Hello from userspace!");
//     Ok(())
// }
//
// fn syscall_info_os_version(_arg_base: u64, _arg_len: u64) -> Result<(), UserError> {
//     unimplemented!();
// }
//
// fn syscall_mem_alloc(_arg_base: u64, arg_len: u64) -> Result<u64, UserError> {
//     let layout = Layout::unpack_u64(arg_len)?;
//     Ok(program::with_current_program_allocator(|alloc| unsafe { alloc.alloc(layout) }) as u64)
// }
//
// fn syscall_mem_dealloc(arg_base: u64, arg_len: u64) -> Result<(), UserError> {
//     let ptr = arg_base as *mut u8;
//     let layout = Layout::unpack_u64(arg_len)?;
//     program::with_current_program_allocator(|alloc| unsafe { alloc.dealloc(ptr, layout) });
//     Ok(())
// }
//
// fn syscall_mem_alloc_zeroed(_arg_base: u64, arg_len: u64) -> Result<u64, UserError> {
//     let layout = Layout::unpack_u64(arg_len)?;
//     Ok(
//         program::with_current_program_allocator(|alloc| unsafe { alloc.alloc_zeroed(layout) })
//             as u64,
//     )
// }
//
// fn syscall_mem_realloc(_arg_base: u64, _arg_len: u64) -> Result<(), UserError> {
//     unimplemented!();
// }
//
// fn syscall_program_exit(_arg_base: u64, _arg_len: u64) -> ! {
//     program::current_program_exit();
// }
//
// fn syscall_program_panic(arg_base: u64, arg_len: u64) -> ! {
//     let info = unsafe { core::slice::from_raw_parts(arg_base as *const u8, arg_len as usize) };
//     let info = core::str::from_utf8(info).unwrap();
//     log::warn!("Program aborted: {}", info);
//     program::current_program_exit();
// }
//
// fn syscall_program_load(_arg_base: u64, _arg_len: u64, user_stack: u64) -> ! {
//     program::save_current_user_stack(user_stack);
//     unimplemented!();
// }
//
// fn syscall_program_wait_for_confirm(_arg_base: u64, _arg_len: u64) -> Result<(), UserError> {
//     program::current_program_wait();
//     Ok(())
// }
//
// fn syscall_screen_create(arg_base: u64, _arg_len: u64) -> Result<(), UserError> {
//     let arg = bool::unpack_u64(arg_base)?;
//     program::create_screen(arg)
// }
//
// fn syscall_screen_set_char(arg_base: u64, arg_len: u64) -> Result<(), UserError> {
//     let (x, y) = <(u32, u32)>::unpack_u64(arg_base)?;
//     let (ch, color) = <(u32, u32)>::unpack_u64(arg_len)?;
//     program::set_screen_char(x as usize, y as usize, ch as u8, color as u8)
// }
//
// fn syscall_screen_set_pixel(arg_base: u64, arg_len: u64) -> Result<(), UserError> {
//     let (x, y) = <(u32, u32)>::unpack_u64(arg_base)?;
//     let color = Color::unpack_u64(arg_len)?;
//     program::set_screen_pixel(x as usize, y as usize, color)
// }
