use crate::{pack_u32s, syscall, SystemError};
use kernel_common::Syscall;

pub use kernel_common::Color;

pub fn create(image: bool) -> Result<(), SystemError> {
    syscall(Syscall::ScreenCreate, if image { 1 } else { 0 }, 0).map(|_| ())
}

pub fn set_char(x: usize, y: usize, ch: u8, color: u8) -> Result<(), SystemError> {
    let arg_pos = pack_u32s(x as u64, y as u64);
    let arg_data = [ch, color, 0, 0, 0, 0, 0, 0];
    syscall(
        Syscall::ScreenSetChar,
        arg_pos,
        u64::from_ne_bytes(arg_data),
    )
    .map(|_| ())
}

pub fn set_pixel(x: usize, y: usize, color: Color) -> Result<(), SystemError> {
    let color = color.to_tuple();
    let arg_pos = pack_u32s(x as u64, y as u64);
    let arg_data = [color.0, color.1, color.2, 0, 0, 0, 0, 0];
    syscall(
        Syscall::ScreenSetPixel,
        arg_pos,
        u64::from_ne_bytes(arg_data),
    )
    .map(|_| ())
}
