use alloc::vec::Vec;
use core::alloc::GlobalAlloc;
use x86_64::VirtAddr;
use fat32::dir::DirError;
use kernel_common::UserError;
use crate::{elf_loader, userspace, memory::*, screen::*, filesystem::get_filesystem};

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
}

impl UserProgram {
    fn new(context: MemoryContext) -> UserProgram {
        UserProgram { context, stack: 0, has_screen: false }
    }
}

enum Screen {
    Text(TextScreen),
    Image(ImageScreen),
}

static PROGRAM_STACK: spin::Mutex<Vec<UserProgram>> = spin::Mutex::new(Vec::new());
static SCREEN_STACK: spin::Mutex<Vec<Screen>> = spin::Mutex::new(Vec::new());

fn push_program(program: UserProgram) {
    PROGRAM_STACK.lock().push(program);
}

fn pop_program() {
    // TODO reclaim memory used by the program
    let program = PROGRAM_STACK.lock().pop().unwrap();
    if program.has_screen {
        SCREEN_STACK.lock().pop();
    }
}

pub fn load_program(program_file: &str) -> Result<VirtAddr, ProgramError> {
    log::info!("Loading program {}", program_file);
    let filesystem = get_filesystem().ok_or(ProgramError::FilesystemMissing)?;
    let file = filesystem.root_dir().cd("programs")?.open_file(program_file)?;
    let mut user_mapper = UserMemoryMapper::init().map_err(|_| ProgramError::MemoryMappingFailed)?;
    let (user_entry, _tls_template) = elf_loader::load_from_disk(&mut user_mapper, file).map_err(|err| ProgramError::ElfError(err))?;
    let context = user_mapper.finish_load();
    push_program(UserProgram::new(context));
    log::debug!("  entry point:{:#X}", user_entry);
    Ok(user_entry)
}

pub fn save_current_user_stack(stack: u64) {
    let mut program_stack = PROGRAM_STACK.lock();
    let current_program = program_stack.last_mut().unwrap();
    current_program.stack = stack;
}

pub fn current_program_exit() -> ! {
    pop_program();
    let mut program_stack = PROGRAM_STACK.lock();
    if let Some(current_program) = program_stack.last_mut() {
        UserMemoryMapper::restore_context(&current_program.context).unwrap();
        userspace::restore_userspace(current_program.stack);
    } else {
        // All programs have exited, shut down the system.
        log::info!("Shutting down");
        crate::logger::show_kernel_screen();
        // TODO
        crate::hlt_loop();
    }
}

pub fn with_current_program_allocator<F, R>(func: F) -> R
        where F: FnOnce(&mut dyn GlobalAlloc) -> R {
    let mut program_stack = PROGRAM_STACK.lock();
    let current_program = program_stack.last_mut().unwrap();
    func(&mut current_program.context.allocator)
}

fn push_screen(screen: Screen) -> Result<(), UserError> {
    let mut program_stack = PROGRAM_STACK.lock();
    let current_program = program_stack.last_mut().unwrap();
    if current_program.has_screen {
        return Err(UserError::HasExistingScreen)
    }
    current_program.has_screen = true;
    SCREEN_STACK.lock().push(screen);
    Ok(())
}

fn pop_screen() -> Result<(), UserError> {
    let mut program_stack = PROGRAM_STACK.lock();
    let current_program = program_stack.last_mut().unwrap();
    if !current_program.has_screen {
        return Err(UserError::MissingScreen)
    }
    current_program.has_screen = false;
    SCREEN_STACK.lock().pop();
    Ok(())
}