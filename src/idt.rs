use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use pic8259::ChainedPics;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use spin;

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

const PIC_OFFSET: u8 = 32;
static PICS: spin::Mutex<ChainedPics> = spin::Mutex::new(unsafe { ChainedPics::new(PIC_OFFSET, PIC_OFFSET + 8) });

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer        = PIC_OFFSET + 0,
    Keyboard     = PIC_OFFSET + 1,
    PrimaryAta   = PIC_OFFSET + 14,
    SecondaryAta = PIC_OFFSET + 15,
}

impl InterruptIndex {
    #[inline(always)]
    fn end_interrupt(self) {
        unsafe { PICS.lock().notify_end_of_interrupt(self as u8); }
    }
}

static KEYBOARD: spin::Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> = spin::Mutex::new(Keyboard::new(HandleControl::Ignore));

pub fn init_idt() {
    unsafe {
        // Exceptions
        IDT.divide_error.set_handler_fn(divide_error_handler).set_stack_index(0);
        IDT.breakpoint.set_handler_fn(breakpoint_handler).set_stack_index(0);
        IDT.overflow.set_handler_fn(overflow_handler).set_stack_index(0);
        IDT.bound_range_exceeded.set_handler_fn(bound_range_exceeded_handler).set_stack_index(0);
        IDT.invalid_opcode.set_handler_fn(invalid_opcode_handler).set_stack_index(0);
        IDT.device_not_available.set_handler_fn(device_not_available_handler).set_stack_index(0);
        IDT.double_fault.set_handler_fn(double_fault_handler).set_stack_index(1);
        IDT.invalid_tss.set_handler_fn(invalid_tss_handler).set_stack_index(0);
        IDT.segment_not_present.set_handler_fn(segment_not_present_handler).set_stack_index(0);
        IDT.stack_segment_fault.set_handler_fn(stack_segment_fault_handler).set_stack_index(0);
        IDT.general_protection_fault.set_handler_fn(general_protection_fault_handler).set_stack_index(0);
        IDT.page_fault.set_handler_fn(page_fault_handler).set_stack_index(0);
        IDT.alignment_check.set_handler_fn(alignment_check_handler).set_stack_index(0);
        IDT.simd_floating_point.set_handler_fn(simd_floating_point_handler).set_stack_index(0);

        // Interrupts
        IDT[InterruptIndex::Timer as usize].set_handler_fn(timer_interrupt_handler).set_stack_index(0);
        IDT[InterruptIndex::Keyboard as usize].set_handler_fn(keyboard_interrupt_handler).set_stack_index(0);
        IDT[InterruptIndex::PrimaryAta as usize].set_handler_fn(primary_ata_interrupt_handler).set_stack_index(0);
        IDT[InterruptIndex::SecondaryAta as usize].set_handler_fn(secondary_ata_interrupt_handler).set_stack_index(0);

        IDT.load();
    }
}
pub fn init_interrupts() {
    unsafe { PICS.lock().initialize() };

    x86_64::instructions::interrupts::enable();

    // The keyboard won't send new interrupts if there is a scancode pending. Read and discard the
    // scancode here in case the user was mashing keys during setup.
    use x86_64::instructions::port::Port;
    let mut port = Port::new(0x60);
    let _scancode: u8 = unsafe { port.read() };
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    InterruptIndex::Timer.end_interrupt();
}
extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;
    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => log::trace!("Keyboard:{}", character),
                DecodedKey::RawKey(key) => log::trace!("Keyboard:{:?}", key),
            }
        }
    }
    InterruptIndex::Keyboard.end_interrupt();
}
extern "x86-interrupt" fn primary_ata_interrupt_handler(_stack_frame: InterruptStackFrame) {
    InterruptIndex::PrimaryAta.end_interrupt();
}
extern "x86-interrupt" fn secondary_ata_interrupt_handler(_stack_frame: InterruptStackFrame) {
    InterruptIndex::SecondaryAta.end_interrupt();
}

fn log_exception_stack_frame(stack_frame: InterruptStackFrame) {
    log::debug!("  at {:#X}", stack_frame.instruction_pointer);
    log::debug!("  stack {:#X}", stack_frame.stack_pointer);
}

extern "x86-interrupt" fn divide_error_handler(stack_frame: InterruptStackFrame) {
    log::error!("EXCEPTION: {}", "DIVIDE BY 0");
    log_exception_stack_frame(stack_frame);
}
extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    log::error!("EXCEPTION: {}", "BREAKPOINT");
    log_exception_stack_frame(stack_frame);
}
extern "x86-interrupt" fn overflow_handler(stack_frame: InterruptStackFrame) {
    log::error!("EXCEPTION: {}", "OVERFLOW");
    log_exception_stack_frame(stack_frame);
}
extern "x86-interrupt" fn bound_range_exceeded_handler(stack_frame: InterruptStackFrame) {
    log::error!("EXCEPTION: {}", "BOUND RANGE EXCEEDED");
    log_exception_stack_frame(stack_frame);
}
extern "x86-interrupt" fn invalid_opcode_handler(stack_frame: InterruptStackFrame) {
    log::error!("EXCEPTION: {}", "INVALID OPCODE");
    log_exception_stack_frame(stack_frame);
    crate::hlt_loop();
}
extern "x86-interrupt" fn device_not_available_handler(stack_frame: InterruptStackFrame) {
    log::error!("EXCEPTION: {}", "DEVICE NOT AVAILABLE");
    log_exception_stack_frame(stack_frame);
}
extern "x86-interrupt" fn double_fault_handler(stack_frame: InterruptStackFrame, _error_code: u64) -> ! {
    log::error!("EXCEPTION: {}", "DOUBLE FAULT");
    log_exception_stack_frame(stack_frame);
    crate::hlt_loop();
}
extern "x86-interrupt" fn invalid_tss_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    log::error!("EXCEPTION: {}({})", "INVALID TSS", error_code);
    log_exception_stack_frame(stack_frame);
}
extern "x86-interrupt" fn segment_not_present_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    log::error!("EXCEPTION: {}({})", "SEGMENT NOT PRESENT", error_code);
    log_exception_stack_frame(stack_frame);
}
extern "x86-interrupt" fn stack_segment_fault_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    log::error!("EXCEPTION: {}({})", "STACK SEGMENT FAULT", error_code);
    log_exception_stack_frame(stack_frame);
}
extern "x86-interrupt" fn general_protection_fault_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    log::error!("EXCEPTION: {}({})", "GENERAL PROTECTION FAULT", error_code);
    log_exception_stack_frame(stack_frame);
    crate::hlt_loop();
}
extern "x86-interrupt" fn page_fault_handler(stack_frame: InterruptStackFrame, error_code: PageFaultErrorCode) {
    let addr = x86_64::registers::control::Cr2::read();
    log::error!("EXCEPTION: {}({:b})", "PAGE FAULT", error_code.bits());
    log_exception_stack_frame(stack_frame);
    log::debug!("  {} {:#X}", if error_code.contains(PageFaultErrorCode::CAUSED_BY_WRITE) { "write" } else { "read" }, addr);
    crate::hlt_loop();
}
extern "x86-interrupt" fn alignment_check_handler(stack_frame: InterruptStackFrame, _error_code: u64) {
    log::error!("EXCEPTION: {}", "ALIGNMENT CHECK");
    log_exception_stack_frame(stack_frame);
}
extern "x86-interrupt" fn simd_floating_point_handler(stack_frame: InterruptStackFrame) {
    log::error!("EXCEPTION: {}", "SIMD FLOATING POINT");
    log_exception_stack_frame(stack_frame);
}
