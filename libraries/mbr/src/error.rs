
/// A general error thrown by the `mbr-nostd` crate.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct MbrError {

    /// The cause of the error, including cause-specific metadata.
    pub cause: ErrorCause,
}

impl MbrError {
    /// Creates a new error from a particular cause.
    pub fn from_cause(cause: ErrorCause) -> MbrError {
        MbrError { cause }
    }
}

/// The possible causes of an error.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ErrorCause {

    /// The error was thrown because we could not determine a partition's type
    /// in the MBR.
    UnsupportedPartitionError{

        /// The unsupported partition type byte read from the table
        tag : u8
    },

    /// The error was thrown because a passed in byte buffer did not end in a valid
    /// MBR suffix of `0x55aa`. 
    InvalidMBRSuffix{

        /// The final 2 bytes of the passed-in raw MBR data
        actual : [u8 ; 2]
    },

    /// The error was thrown because a passed-in buffer did not match a size 
    /// requirement.
    BufferWrongSizeError{

        /// The size of the buffer that the function expected
        expected : usize ,

        /// The size of the buffer passed into the function
        actual : usize
    },
}