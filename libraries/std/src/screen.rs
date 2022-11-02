use crate::{syscall, SyscallArg, SystemError};
use kernel_common::Syscall;

pub use kernel_common::Color;

pub fn create(image: bool) -> Result<(), SystemError> {
    syscall(Syscall::ScreenCreate, bool::pack_u64(image), 0).map(|_| ())
}

pub fn set_char(x: usize, y: usize, ch: u8, color: u8) -> Result<(), SystemError> {
    let arg_pos = (x as u32, y as u32).pack_u64();
    let arg_data = (ch as u32, color as u32).pack_u64();
    syscall(Syscall::ScreenSetChar, arg_pos, arg_data).map(|_| ())
}

pub fn set_pixel(x: usize, y: usize, color: Color) -> Result<(), SystemError> {
    let arg_pos = (x as u32, y as u32).pack_u64();
    let arg_data = color.pack_u64();
    syscall(Syscall::ScreenSetPixel, arg_pos, arg_data).map(|_| ())
}
