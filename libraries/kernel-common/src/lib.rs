#![no_std]

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u64)]
pub enum UserError {
    InvalidValue = Self::INVALID_VALUE,
    MissingScreen = Self::MISSING_SCREEN,
    HasExistingScreen = Self::HAS_EXISTING_SCREEN,
    ScreenWrongType = Self::SCREEN_WRONG_TYPE,
}

impl UserError {
    const BASE_VALUE: u64 = u64::MAX - 16;
    const INVALID_VALUE: u64 = Self::BASE_VALUE;
    const MISSING_SCREEN: u64 = Self::BASE_VALUE + 1;
    const HAS_EXISTING_SCREEN: u64 = Self::BASE_VALUE + 2;
    const SCREEN_WRONG_TYPE: u64 = Self::BASE_VALUE + 3;
}

impl UserError {
    pub fn pack(result: Result<u64, UserError>) -> u64 {
        match result {
            Ok(ok) => ok,
            Err(err) => err as u64,
        }
    }
    pub fn unpack(value: u64) -> Result<u64, UserError> {
        if value >= UserError::BASE_VALUE {
            Err(value.try_into().unwrap_or(UserError::InvalidValue))
        } else {
            Ok(value)
        }
    }
}

impl TryFrom<u64> for UserError {
    type Error = ();
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            UserError::INVALID_VALUE => Ok(UserError::InvalidValue),
            UserError::MISSING_SCREEN => Ok(UserError::MissingScreen),
            UserError::HAS_EXISTING_SCREEN => Ok(UserError::HasExistingScreen),
            _ => Err(()),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u64)]
pub enum Syscall {
    InfoOsName = 0x0100,
    InfoOsVersion,
    MemAlloc = 0x0200,
    MemDealloc,
    MemAllocZeroed,
    MemRealloc,
    ProgramExit = 0x0300,
    ProgramPanic,
    ProgramLoad,
    ScreenCreate = 0x0400,
    ScreenSetChar,
    ScreenSetPixel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(u8, u8, u8);

impl Color {
    pub const BLACK: Color = Color(0, 0, 0);
    pub fn new(r: u8, g: u8, b: u8) -> Color {
        Color(r, g, b)
    }
    pub fn to_tuple(self) -> (u8, u8, u8) {
        (self.0, self.1, self.2)
    }
}
