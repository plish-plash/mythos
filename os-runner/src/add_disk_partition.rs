use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use mbr::*;

const SECTOR_SIZE: usize = 512;

pub fn modify_file(base_file: &Path, partition_file: &Path) -> std::io::Result<()> {
    // Read files
    let mut buffer = {
        let mut reader = BufReader::new(File::open(base_file)?);
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;
        buffer
    };
    let mut partition_data = {
        let mut reader = BufReader::new(File::open(partition_file)?);
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;
        buffer
    };

    // Pad to a whole number of sectors
    while buffer.len() % SECTOR_SIZE != 0 {
        buffer.push(0);
    }
    while partition_data.len() % SECTOR_SIZE != 0 {
        partition_data.push(0);
    }
    println!("Kernel image size: {}", buffer.len());
    println!("User partition size: {}", partition_data.len());

    // Modify mbr (currently empty)
    let mut mbr = MasterBootRecord::from_bytes(&buffer).unwrap();
    {
        let entry = &mut mbr.entries[0];
        entry.bootable = true;
        entry.partition_type = PartitionType::from_mbr_tag_byte(0x01);
        entry.logical_block_address = 0;
        entry.sector_count = (buffer.len() / SECTOR_SIZE) as u32;
    }
    {
        let entry = &mut mbr.entries[1];
        entry.bootable = false;
        entry.partition_type = PartitionType::from_mbr_tag_byte(0x0b);
        entry.logical_block_address = (buffer.len() / SECTOR_SIZE) as u32;
        entry.sector_count = (partition_data.len() / SECTOR_SIZE) as u32;
    }
    mbr.serialize(&mut buffer).unwrap();

    // Save combined image
    buffer.append(&mut partition_data);
    BufWriter::new(File::create(base_file)?).write_all(&buffer)?;

    Ok(())
}
