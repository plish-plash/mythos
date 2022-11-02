use crate::{elf_loader, filesystem::get_filesystem, memory::*, screen::*, userspace};
use alloc::vec::Vec;
use core::alloc::GlobalAlloc;
use fat32::dir::DirError;
use kernel_common::{Color, UserError};
use uniquelock::UniqueLock;
use x86_64::VirtAddr;

#[derive(Debug)]
pub enum ProgramError {
    MemoryMappingFailed,
    FilesystemMissing,
    FilesystemError(DirError),
    ElfError(&'static str),
}

impl From<DirError> for ProgramError {
    fn from(err: DirError) -> Self {
        ProgramError::FilesystemError(err)
    }
}

struct UserProgram {
    context: MemoryContext,
    stack: u64,
    has_screen: bool,
    confirm: bool,
}

impl UserProgram {
    fn new(context: MemoryContext) -> UserProgram {
        UserProgram {
            context,
            stack: 0,
            has_screen: false,
            confirm: false,
        }
    }
}

enum Screen {
    Text(TextScreen),
    Image(ImageScreen),
}

impl Screen {
    fn set_active(&mut self, active: bool) {
        use crate::screen::Screen as ScreenTrait;
        match self {
            Screen::Text(screen) => screen.set_active(active),
            Screen::Image(screen) => screen.set_active(active),
        }
    }
}

static PROGRAM_STACK: UniqueLock<Vec<UserProgram>> = UniqueLock::new("program stack", Vec::new());
static SCREEN_STACK: UniqueLock<Vec<Screen>> = UniqueLock::new("screen stack", Vec::new());

fn push_program(program: UserProgram) {
    PROGRAM_STACK.lock().unwrap().push(program);
}

fn pop_program() {
    // TODO reclaim memory used by the program
    let program = PROGRAM_STACK.lock().unwrap().pop().unwrap();
    if program.has_screen {
        SCREEN_STACK.lock().unwrap().pop();
        set_screen_active(true);
    }
}

pub fn load_program(program_file: &str) -> Result<VirtAddr, ProgramError> {
    log::info!("Loading program {}", program_file);
    let filesystem = get_filesystem().ok_or(ProgramError::FilesystemMissing)?;
    let file = filesystem
        .root_dir()
        .cd("programs")?
        .open_file(program_file)?;
    let mut user_mapper =
        UserMemoryMapper::init().map_err(|_| ProgramError::MemoryMappingFailed)?;
    let (user_entry, _tls_template) =
        elf_loader::load_from_disk(&mut user_mapper, file).map_err(ProgramError::ElfError)?;
    let context = user_mapper.finish_load();
    push_program(UserProgram::new(context));
    log::debug!("  entry point:{:#X}", user_entry);
    Ok(user_entry)
}

pub fn save_current_user_stack(stack: u64) {
    let mut program_stack = PROGRAM_STACK.lock().unwrap();
    let current_program = program_stack.last_mut().unwrap();
    current_program.stack = stack;
}

pub fn current_program_exit() -> ! {
    pop_program();
    let mut program_stack = PROGRAM_STACK.lock().unwrap();
    if let Some(current_program) = program_stack.last_mut() {
        UserMemoryMapper::restore_context(&current_program.context).unwrap();
        userspace::restore_userspace(current_program.stack);
    } else {
        // All programs have exited, shut down the system.
        log::info!("Shutting down");
        // TODO
        crate::hlt_loop();
    }
}

pub fn current_program_wait() {
    {
        let mut program_stack = PROGRAM_STACK.lock().unwrap();
        let current_program = program_stack.last_mut().unwrap();
        current_program.confirm = false;
    }
    let mut confirm = false;
    while !confirm {
        x86_64::instructions::hlt();
        if let Ok(program_stack) = PROGRAM_STACK.lock() {
            confirm = program_stack.last().unwrap().confirm;
        }
    }
}

pub fn current_program_notify() -> bool {
    if let Ok(mut program_stack) = PROGRAM_STACK.lock() {
        if let Some(current_program) = program_stack.last_mut() {
            current_program.confirm = true;
            return true;
        }
    }
    false
}

pub fn with_current_program_allocator<F, R>(func: F) -> R
where
    F: FnOnce(&mut dyn GlobalAlloc) -> R,
{
    let mut program_stack = PROGRAM_STACK.lock().unwrap();
    let current_program = program_stack.last_mut().unwrap();
    func(&mut current_program.context.allocator)
}

fn set_screen_active(active: bool) {
    let mut screen_stack = SCREEN_STACK.lock().unwrap();
    if let Some(screen) = screen_stack.last_mut() {
        screen.set_active(active);
    } else {
        crate::logger::show_kernel_screen(active);
    }
}

fn push_screen(screen: Screen) -> Result<(), UserError> {
    let mut program_stack = PROGRAM_STACK.lock().unwrap();
    let current_program = program_stack.last_mut().unwrap();
    if current_program.has_screen {
        return Err(UserError::HasExistingScreen);
    }
    current_program.has_screen = true;
    set_screen_active(false);
    SCREEN_STACK.lock().unwrap().push(screen);
    set_screen_active(true);
    Ok(())
}

fn pop_screen() -> Result<(), UserError> {
    let mut program_stack = PROGRAM_STACK.lock().unwrap();
    let current_program = program_stack.last_mut().unwrap();
    if !current_program.has_screen {
        return Err(UserError::MissingScreen);
    }
    current_program.has_screen = false;
    SCREEN_STACK.lock().unwrap().pop();
    set_screen_active(true);
    Ok(())
}

fn make_user_text_palette() -> Palette {
    unimplemented!(); // TODO
}

pub fn create_screen(image: bool) -> Result<(), UserError> {
    let screen = if image {
        Screen::Image(ImageScreen::new(Color::BLACK))
    } else {
        Screen::Text(TextScreen::new(make_user_text_palette()))
    };
    push_screen(screen)
}

pub fn set_screen_char(x: usize, y: usize, ch: u8, color: u8) -> Result<(), UserError> {
    let program_stack = PROGRAM_STACK.lock().unwrap();
    let current_program = program_stack.last().unwrap();
    if !current_program.has_screen {
        return Err(UserError::MissingScreen);
    }
    let mut screen_stack = SCREEN_STACK.lock().unwrap();
    match screen_stack.last_mut().unwrap() {
        Screen::Text(screen) => {
            screen.set_char(x, y, ch, PaletteColor::new(color));
            Ok(())
        }
        Screen::Image(_) => Err(UserError::ScreenWrongType),
    }
}

pub fn set_screen_pixel(x: usize, y: usize, r: u8, g: u8, b: u8) -> Result<(), UserError> {
    let program_stack = PROGRAM_STACK.lock().unwrap();
    let current_program = program_stack.last().unwrap();
    if !current_program.has_screen {
        return Err(UserError::MissingScreen);
    }
    let mut screen_stack = SCREEN_STACK.lock().unwrap();
    match screen_stack.last_mut().unwrap() {
        Screen::Text(_) => Err(UserError::ScreenWrongType),
        Screen::Image(screen) => {
            screen.set_pixel(x, y, Color::new(r, g, b));
            Ok(())
        }
    }
}
