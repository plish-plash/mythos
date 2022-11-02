use crate::{memory, program};
use core::alloc::Layout;
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
    match id {
        Syscall::InfoOsName => {
            // TODO
            log::info!("Hello from userspace!");
        }
        Syscall::InfoOsVersion => unimplemented!(),
        Syscall::MemAlloc => {
            return program::with_current_program_allocator(|alloc| {
                let layout = Layout::unpack_u64(arg_len)?;
                unsafe { Ok(alloc.alloc(layout) as u64) }
            })
            .into()
        }
        Syscall::MemDealloc => {
            return program::with_current_program_allocator(|alloc| {
                let layout = Layout::unpack_u64(arg_len)?;
                unsafe {
                    alloc.dealloc(arg_base as *mut u8, layout);
                }
                Ok(())
            })
            .into()
        }
        Syscall::MemAllocZeroed => {
            return program::with_current_program_allocator(|alloc| {
                let layout = Layout::unpack_u64(arg_len)?;
                unsafe { Ok(alloc.alloc_zeroed(layout) as u64) }
            })
            .into()
        }
        Syscall::MemRealloc => unimplemented!(),
        Syscall::ProgramExit => program::current_program_exit(),
        Syscall::ProgramPanic => {
            let info =
                unsafe { core::slice::from_raw_parts(arg_base as *const u8, arg_len as usize) };
            let info = core::str::from_utf8(info).unwrap();
            log::warn!("Program aborted: {}", info);
            crate::logger::show_kernel_screen(true);
            program::current_program_exit();
        }
        Syscall::ProgramLoad => unimplemented!(),
        Syscall::ProgramWaitForConfirm => program::current_program_wait(),
        Syscall::ScreenCreate => {
            return bool::unpack_u64(arg_base)
                .and_then(program::create_screen)
                .into()
        }
        Syscall::ScreenSetChar => {
            return (|| {
                let (x, y) = <(u32, u32)>::unpack_u64(arg_base)?;
                let (ch, color) = <(u32, u32)>::unpack_u64(arg_len)?;
                program::set_screen_char(x as usize, y as usize, ch as u8, color as u8)
            })()
            .into()
        }
        Syscall::ScreenSetPixel => {
            return (|| {
                let (x, y) = <(u32, u32)>::unpack_u64(arg_base)?;
                let color = Color::unpack_u64(arg_len)?;
                program::set_screen_pixel(x as usize, y as usize, color)
            })()
            .into()
        }
    };
    Ok(()).into()
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
