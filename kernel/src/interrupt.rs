use crate::fatal_error;
use pc_keyboard::{layouts, HandleControl, Keyboard, ScancodeSet1};
use pic8259::ChainedPics;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

static mut IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

const PIC_OFFSET: u8 = 32;
static mut PICS: ChainedPics = unsafe { ChainedPics::new(PIC_OFFSET, PIC_OFFSET + 8) };

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_OFFSET + 0,
    Keyboard = PIC_OFFSET + 1,
    PrimaryAta = PIC_OFFSET + 14,
    SecondaryAta = PIC_OFFSET + 15,
}

impl InterruptIndex {
    #[inline(always)]
    fn end_interrupt(self) {
        unsafe {
            PICS.notify_end_of_interrupt(self as u8);
        }
    }
}

static mut KEYBOARD: Keyboard<layouts::Us104Key, ScancodeSet1> = Keyboard::new(
    ScancodeSet1::new(),
    layouts::Us104Key,
    HandleControl::Ignore,
);

pub fn init_idt() {
    unsafe {
        // Exceptions
        IDT.divide_error
            .set_handler_fn(divide_error_handler)
            .set_stack_index(0);
        IDT.breakpoint
            .set_handler_fn(breakpoint_handler)
            .set_stack_index(0);
        IDT.overflow
            .set_handler_fn(overflow_handler)
            .set_stack_index(0);
        IDT.bound_range_exceeded
            .set_handler_fn(bound_range_exceeded_handler)
            .set_stack_index(0);
        IDT.invalid_opcode
            .set_handler_fn(invalid_opcode_handler)
            .set_stack_index(0);
        IDT.device_not_available
            .set_handler_fn(device_not_available_handler)
            .set_stack_index(0);
        IDT.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(1);
        IDT.invalid_tss
            .set_handler_fn(invalid_tss_handler)
            .set_stack_index(0);
        IDT.segment_not_present
            .set_handler_fn(segment_not_present_handler)
            .set_stack_index(0);
        IDT.stack_segment_fault
            .set_handler_fn(stack_segment_fault_handler)
            .set_stack_index(0);
        IDT.general_protection_fault
            .set_handler_fn(general_protection_fault_handler)
            .set_stack_index(0);
        IDT.page_fault
            .set_handler_fn(page_fault_handler)
            .set_stack_index(0);
        IDT.alignment_check
            .set_handler_fn(alignment_check_handler)
            .set_stack_index(0);
        IDT.simd_floating_point
            .set_handler_fn(simd_floating_point_handler)
            .set_stack_index(0);

        // Interrupts
        IDT[InterruptIndex::Timer as usize]
            .set_handler_fn(timer_interrupt_handler)
            .set_stack_index(0);
        IDT[InterruptIndex::Keyboard as usize]
            .set_handler_fn(keyboard_interrupt_handler)
            .set_stack_index(0);
        IDT[InterruptIndex::PrimaryAta as usize]
            .set_handler_fn(primary_ata_interrupt_handler)
            .set_stack_index(0);
        IDT[InterruptIndex::SecondaryAta as usize]
            .set_handler_fn(secondary_ata_interrupt_handler)
            .set_stack_index(0);

        IDT.load();
    }
}
pub fn init_interrupts() {
    use x86_64::instructions::port::Port;
    unsafe {
        PICS.initialize();
    }

    // Configure timer.
    let timer_rate = 19853_u16; // 60.1 Hz
    let mut timer_command_port = Port::new(0x43);
    let mut timer_data_port = Port::new(0x40);
    unsafe {
        timer_command_port.write(0b00110100_u8); // channel 0, lobyte/hibyte, rate generator
        timer_data_port.write((timer_rate & 0xFF) as u8); // divider lobyte
        timer_data_port.write(((timer_rate >> 8) & 0xFF) as u8); // divider hibyte
    }

    x86_64::instructions::interrupts::enable();

    // The keyboard won't send new interrupts if there is a scancode pending. Read and discard the
    // scancode here in case the user was mashing keys during setup.
    unsafe {
        Port::<u8>::new(0x60).read();
    }
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // unsafe {
    //     crate::game::WAIT_FRAME = false;
    // }
    // TODO
    InterruptIndex::Timer.end_interrupt();
}
extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    if let Ok(Some(key_event)) = unsafe { KEYBOARD.add_byte(scancode) } {
        if let Some(key) = unsafe { KEYBOARD.process_keyevent(key_event) } {
            match key {
                // DecodedKey::Unicode(character) => log::trace!("Keyboard:{}", character),
                // DecodedKey::RawKey(key) => log::trace!("Keyboard:{:?}", key),
                _ => (),
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

extern "x86-interrupt" fn divide_error_handler(_stack_frame: InterruptStackFrame) {
    fatal_error!("EXCEPTION: {}", "DIVIDE BY 0");
}
extern "x86-interrupt" fn breakpoint_handler(_stack_frame: InterruptStackFrame) {
    fatal_error!("EXCEPTION: {}", "BREAKPOINT");
}
extern "x86-interrupt" fn overflow_handler(_stack_frame: InterruptStackFrame) {
    fatal_error!("EXCEPTION: {}", "OVERFLOW");
}
extern "x86-interrupt" fn bound_range_exceeded_handler(_stack_frame: InterruptStackFrame) {
    fatal_error!("EXCEPTION: {}", "BOUND RANGE EXCEEDED");
}
extern "x86-interrupt" fn invalid_opcode_handler(_stack_frame: InterruptStackFrame) {
    fatal_error!("EXCEPTION: {}", "INVALID OPCODE");
}
extern "x86-interrupt" fn device_not_available_handler(_stack_frame: InterruptStackFrame) {
    fatal_error!("EXCEPTION: {}", "DEVICE NOT AVAILABLE");
}
extern "x86-interrupt" fn double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    fatal_error!("EXCEPTION: {}", "DOUBLE FAULT");
}
extern "x86-interrupt" fn invalid_tss_handler(_stack_frame: InterruptStackFrame, error_code: u64) {
    fatal_error!("EXCEPTION: {}({})", "INVALID TSS", error_code);
}
extern "x86-interrupt" fn segment_not_present_handler(
    _stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    fatal_error!("EXCEPTION: {}({})", "SEGMENT NOT PRESENT", error_code);
}
extern "x86-interrupt" fn stack_segment_fault_handler(
    _stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    fatal_error!("EXCEPTION: {}({})", "STACK SEGMENT FAULT", error_code);
}
extern "x86-interrupt" fn general_protection_fault_handler(
    _stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    fatal_error!("EXCEPTION: {}({})", "GENERAL PROTECTION FAULT", error_code);
}
extern "x86-interrupt" fn page_fault_handler(
    _stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let fault_address = x86_64::registers::control::Cr2::read();
    fatal_error!(
        "EXCEPTION: {}({:06b}) {:#x}",
        "PAGE FAULT",
        error_code,
        fault_address
    );
}
extern "x86-interrupt" fn alignment_check_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) {
    fatal_error!("EXCEPTION: {}", "ALIGNMENT CHECK");
}
extern "x86-interrupt" fn simd_floating_point_handler(_stack_frame: InterruptStackFrame) {
    fatal_error!("EXCEPTION: {}", "SIMD FLOATING POINT");
}
