// Based on https://github.com/ischeinkman/mbr-nostd

#![no_std]

mod error;
pub use error::{ErrorCause, MbrError};

mod partition;
pub use partition::*;

fn read_u32_le(buf: &[u8]) -> u32 {
    u32::from_le_bytes(buf.try_into().unwrap())
}
fn write_u32_le(buf: &mut [u8], val: u32) {
    let bytes = val.to_le_bytes();
    buf.copy_from_slice(&bytes);
}

/// A struct representing an MBR partition table.
pub struct MasterBootRecord {
    pub entries: [PartitionTableEntry; MAX_ENTRIES],
}

const BUFFER_SIZE: usize = 512;
const TABLE_OFFSET: usize = 446;
const ENTRY_SIZE: usize = 16;
const SUFFIX_BYTES: [u8; 2] = [0x55, 0xaa];
const MAX_ENTRIES: usize = (BUFFER_SIZE - TABLE_OFFSET - 2) / ENTRY_SIZE;

impl MasterBootRecord {
    /// Parses the MBR table from a raw byte buffer.
    ///
    /// Throws an error in the following cases:
    /// * `BufferWrongSizeError` if `bytes.len()` is less than 512
    /// * `InvalidMBRSuffix` if the final 2 bytes in `bytes` are not `[0x55, 0xaa]`
    /// * `UnsupportedPartitionError` if the MBR contains a tag that the crate does not recognize
    pub fn from_bytes<T: AsRef<[u8]>>(bytes: &T) -> Result<MasterBootRecord, MbrError> {
        let buffer: &[u8] = bytes.as_ref();
        if buffer.len() < BUFFER_SIZE {
            return Err(MbrError::from_cause(ErrorCause::BufferWrongSizeError {
                expected: BUFFER_SIZE,
                actual: buffer.len(),
            }));
        } else if buffer[BUFFER_SIZE - SUFFIX_BYTES.len()..BUFFER_SIZE] != SUFFIX_BYTES[..] {
            return Err(MbrError::from_cause(ErrorCause::InvalidMBRSuffix {
                actual: [buffer[BUFFER_SIZE - 2], buffer[BUFFER_SIZE - 1]],
            }));
        }
        let mut entries = [PartitionTableEntry::empty(); MAX_ENTRIES];
        for idx in 0..MAX_ENTRIES {
            let offset = TABLE_OFFSET + idx * ENTRY_SIZE;
            let bootable = buffer[offset] != 0;
            let partition_type = PartitionType::from_mbr_tag_byte(buffer[offset + 4]);
            // if let PartitionType::Unknown(c) = partition_type {
            //     return Err(MbrError::from_cause(ErrorCause::UnsupportedPartitionError { tag : c}));
            // }
            let buffer_off = &buffer[offset..];
            let lba = read_u32_le(&buffer_off[8..12]);
            let len = read_u32_le(&buffer_off[12..16]);
            entries[idx] = PartitionTableEntry::new(bootable, partition_type, lba, len);
        }
        Ok(MasterBootRecord { entries })
    }

    /// Serializes this MBR partition table to a raw byte buffer.

    /// Throws an error in the following cases:
    /// * `BufferWrongSizeError` if `buffer.len()` is less than 512
    ///
    /// Note that it only affects the partition table itself, which only appears starting
    /// from byte `446` of the MBR; no bytes before this are affected, even though it is
    /// still necessary to pass a full `512` byte buffer.
    pub fn serialize<T: AsMut<[u8]>>(&self, buffer: &mut T) -> Result<usize, MbrError> {
        let buffer: &mut [u8] = buffer.as_mut();
        if buffer.len() < BUFFER_SIZE {
            return Err(MbrError::from_cause(ErrorCause::BufferWrongSizeError {
                expected: BUFFER_SIZE,
                actual: buffer.len(),
            }));
        }
        {
            let suffix: &mut [u8] = &mut buffer[BUFFER_SIZE - SUFFIX_BYTES.len()..BUFFER_SIZE];
            suffix.copy_from_slice(&SUFFIX_BYTES);
        }
        for idx in 0..MAX_ENTRIES {
            let offset = TABLE_OFFSET + idx * ENTRY_SIZE;
            let entry = self.entries[idx];
            buffer[offset] = if entry.bootable { 0x80 } else { 0x00 };
            buffer[offset + 4] = entry.partition_type.to_mbr_tag_byte();
            {
                let lba_slice: &mut [u8] = &mut buffer[offset + 8..offset + 12];
                write_u32_le(lba_slice, entry.logical_block_address);
            }
            {
                let len_slice: &mut [u8] = &mut buffer[offset + 12..offset + 16];
                write_u32_le(len_slice, entry.sector_count);
            }
        }
        Ok(BUFFER_SIZE)
    }
}
