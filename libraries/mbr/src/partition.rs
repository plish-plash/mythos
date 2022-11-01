/// The type of a particular partition.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum PartitionType {
    Unused,
    Unknown(u8),
    Fat12(u8),
    Fat16(u8),
    Fat32(u8),
    LinuxExt(u8),
    HfsPlus(u8),
    ISO9660(u8),
    NtfsExfat(u8),
}

impl PartitionType {
    /// Parses a partition type from the type byte in the MBR's table.
    pub fn from_mbr_tag_byte(tag: u8) -> PartitionType {
        match tag {
            0x0 => PartitionType::Unused,
            0x01 => PartitionType::Fat12(tag),
            0x04 | 0x06 | 0x0e => PartitionType::Fat16(tag),
            0x0b | 0x0c | 0x1b | 0x1c => PartitionType::Fat32(tag),
            0x83 => PartitionType::LinuxExt(tag),
            0x07 => PartitionType::NtfsExfat(tag),
            0xaf => PartitionType::HfsPlus(tag),
            _ => PartitionType::Unknown(tag),
        }
    }

    /// Retrieves the associated type byte for this partition type.
    pub fn to_mbr_tag_byte(&self) -> u8 {
        match *self {
            PartitionType::Unused => 0,
            PartitionType::Unknown(t) => t,
            PartitionType::Fat12(t) => t,
            PartitionType::Fat16(t) => t,
            PartitionType::Fat32(t) => t,
            PartitionType::LinuxExt(t) => t,
            PartitionType::HfsPlus(t) => t,
            PartitionType::ISO9660(t) => t,
            PartitionType::NtfsExfat(t) => t,
        }
    }
}

/// An entry in a partition table.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct PartitionTableEntry {
    pub bootable: bool,

    /// The type of partition in this entry.
    pub partition_type: PartitionType,

    /// The index of the first block of this entry.
    pub logical_block_address: u32,

    /// The total number of blocks in this entry.
    pub sector_count: u32,
}

impl PartitionTableEntry {
    pub fn new(
        bootable: bool,
        partition_type: PartitionType,
        logical_block_address: u32,
        sector_count: u32,
    ) -> PartitionTableEntry {
        PartitionTableEntry {
            bootable,
            partition_type,
            logical_block_address,
            sector_count,
        }
    }

    pub fn empty() -> PartitionTableEntry {
        PartitionTableEntry::new(false, PartitionType::Unused, 0, 0)
    }
}
