#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(naked_functions)]
#![no_std]
#![no_main]
extern crate alloc;

//mod elf_loader;
//mod filesystem;
mod graphics;
mod interrupt;
mod memory;
//mod program;
//mod screen;
mod game;
mod userspace;

use ata::{AtaError, BlockDevice};
use bootloader_api::{config::Mapping, entry_point, BootInfo, BootloaderConfig};
use core::{fmt::Write, panic::PanicInfo};

use graphics::Texture;

static OS_NAME: &str = "MariOS";
static OS_VERSION: &str = env!("CARGO_PKG_VERSION");

static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.dynamic_range_start = Some(0xd000_0000_0000);
    config.mappings.physical_memory = Some(Mapping::FixedAddress(0xf000_0000_0000));
    config
};

// TODO pretty error messages
#[derive(Debug)]
enum KernelInitError {
    PhysicalMemoryNotMapped,
    AtaError(AtaError),
    AtaNoDrive,
    InvalidDiskMbr,
}

impl From<AtaError> for KernelInitError {
    fn from(err: AtaError) -> Self {
        KernelInitError::AtaError(err)
    }
}

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    if let Some(framebuffer_info) = boot_info.framebuffer.as_mut() {
        graphics::set_framebuffer(framebuffer_info);
    }

    let (context, framebuffer) = graphics::setup_context();
    context.clear(framebuffer);
    graphics::load_system_font(&context);

    let screen_width = framebuffer.width();
    let mut init_writer = graphics::TextWriter::new(&context, framebuffer, 0, 64);
    init_writer.center_x(screen_width, 24);
    writeln!(init_writer, "         {}", OS_NAME).ok();
    writeln!(init_writer, "     Kernel v{}", OS_VERSION).ok();
    writeln!(
        init_writer,
        "   Bootloader v{}.{}.{}",
        boot_info.api_version.version_major(),
        boot_info.api_version.version_minor(),
        boot_info.api_version.version_patch()
    )
    .ok();

    let phys_offset = boot_info
        .physical_memory_offset
        .into_option()
        .ok_or(KernelInitError::PhysicalMemoryNotMapped)
        .unwrap();
    writeln!(init_writer, "       Loading GDT").ok();
    userspace::init_gdt();
    writeln!(init_writer, "       Loading IDT").ok();
    interrupt::init_idt();
    writeln!(init_writer, "Setting up kernel memory").ok();
    memory::init_memory(phys_offset, &boot_info.memory_regions);
    writeln!(init_writer, "   Enabling interrupts").ok();
    interrupt::init_interrupts();

    game::run_game(&context, framebuffer);
    // log::info!("Initializing ATA");
    // let drive_info = get_first_ata_drive().unwrap();
    // log::debug!(
    //     "Found drive {} size:{}KiB",
    //     drive_info.model,
    //     drive_info.size_in_kib()
    // );
    // let user_partition = get_user_partition(drive_info.drive).unwrap();
    // log::debug!("  user partition size:{}KiB", user_partition.size_in_kib());
    // filesystem::init_fs(user_partition);
    // let entry_point = program::load_program("raytrace.elf").unwrap();
    // userspace::enter_userspace(entry_point);
}

fn get_first_ata_drive() -> Result<ata::DriveInfo, KernelInitError> {
    ata::init()?;
    ata::list()?
        .into_iter()
        .next()
        .ok_or(KernelInitError::AtaNoDrive)
}

fn get_user_partition(drive: ata::Drive) -> Result<ata::Partition, KernelInitError> {
    let mut mbr_bytes = alloc::vec![0u8; 512];
    drive.read(&mut mbr_bytes, 0, 1).unwrap();
    let mbr = mbr::MasterBootRecord::from_bytes(&mbr_bytes)
        .map_err(|_| KernelInitError::InvalidDiskMbr)?;
    if mbr.entries[0].partition_type == mbr::PartitionType::Unused
        || mbr.entries[1].partition_type == mbr::PartitionType::Unused
    {
        return Err(KernelInitError::InvalidDiskMbr);
    }
    if !mbr.entries[0].bootable || mbr.entries[0].logical_block_address != 0 {
        return Err(KernelInitError::InvalidDiskMbr);
    }
    Ok(ata::Partition::new(
        drive,
        mbr.entries[1].logical_block_address as usize,
        mbr.entries[1].sector_count as usize,
    ))
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

#[macro_export]
macro_rules! fatal_error {
    ($($arg:tt)*) => {
        let (context, framebuffer) = $crate::graphics::setup_context();
        let mut error_writer = $crate::graphics::TextWriter::new(&context, framebuffer, 0, 0);
        error_writer.write_fmt(format_args!($($arg)*)).ok();
        $crate::hlt_loop();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    fatal_error!("{}", info);
}

#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    fatal_error!("alloc failed: {:?}", layout);
}
