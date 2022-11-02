#![no_std]

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u64)]
pub enum UserError {
    Unknown = 0,
    InvalidValue,
    MissingScreen,
    HasExistingScreen,
    ScreenWrongType,
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
    ProgramWaitForConfirm,
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

// sysv64 ABI can return up to 128 bytes in registers
#[repr(C)]
pub struct SyscallRetValue(u64, u64);

impl From<Result<(), UserError>> for SyscallRetValue {
    fn from(value: Result<(), UserError>) -> Self {
        match value {
            Ok(()) => Self(1, 0),
            Err(err) => Self(0, err as u64),
        }
    }
}

impl From<Result<u64, UserError>> for SyscallRetValue {
    fn from(value: Result<u64, UserError>) -> Self {
        match value {
            Ok(val) => Self(1, val),
            Err(err) => Self(0, err as u64),
        }
    }
}

impl From<Result<(u64, u64), UserError>> for SyscallRetValue {
    fn from(value: Result<(u64, u64), UserError>) -> Self {
        match value {
            Ok((a, b)) => {
                assert_ne!(a, 0);
                Self(a, b)
            }
            Err(err) => Self(0, err as u64),
        }
    }
}

pub trait SyscallArg: Sized {
    fn unpack_u64(value: u64) -> Result<Self, UserError>;
    fn pack_u64(self) -> u64;
}

impl SyscallArg for bool {
    fn unpack_u64(value: u64) -> Result<Self, UserError> {
        match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(UserError::InvalidValue),
        }
    }
    fn pack_u64(self) -> u64 {
        if self {
            1
        } else {
            0
        }
    }
}

impl SyscallArg for (u32, u32) {
    fn unpack_u64(value: u64) -> Result<Self, UserError> {
        Ok(((value >> 32) as u32, (value & u32::MAX as u64) as u32))
    }
    fn pack_u64(self) -> u64 {
        ((self.0 as u64) << 32) | (self.1 as u64)
    }
}

impl SyscallArg for Color {
    fn unpack_u64(value: u64) -> Result<Self, UserError> {
        let r = (value >> 16) & u8::MAX as u64;
        let g = (value >> 8) & u8::MAX as u64;
        let b = value & u8::MAX as u64;
        Ok(Color(r as u8, g as u8, b as u8))
    }
    fn pack_u64(self) -> u64 {
        (self.0 as u64) << 16 | (self.1 as u64) << 8 | (self.2 as u64)
    }
}

impl SyscallArg for core::alloc::Layout {
    fn unpack_u64(value: u64) -> Result<Self, UserError> {
        let (align, size) = <(u32, u32)>::unpack_u64(value)?;
        core::alloc::Layout::from_size_align(size as usize, align as usize)
            .map_err(|_| UserError::InvalidValue)
    }
    fn pack_u64(self) -> u64 {
        let align = self.align();
        let size = self.size();
        (align as u32, size as u32).pack_u64()
    }
}
