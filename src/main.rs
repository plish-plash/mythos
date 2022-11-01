#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(naked_functions)]
#![no_std]
#![no_main]
extern crate alloc;

mod elf_loader;
mod filesystem;
mod graphics;
mod idt;
mod logger;
mod memory;
mod program;
mod screen;
mod userspace;

use ata::BlockDevice;
use bootloader::{boot_info::FrameBufferInfo, entry_point, BootInfo};
use core::panic::PanicInfo;

static OS_NAME: &str = "GenOS";
static OS_VERSION: &str = env!("CARGO_PKG_VERSION");

// TODO pretty error messages
#[derive(Debug)]
enum KernelInitError {
    FramebufferWrongSize,
    PhysicalMemoryNotMapped,
    AtaFailed,
    InvalidDiskMbr,
}

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    if let Some(framebuffer) = boot_info.framebuffer.as_mut() {
        graphics::set_global_framebuffer(framebuffer);
    }

    logger::init().unwrap();
    log::info!("{}", OS_NAME);
    log::info!("Kernel v{}", OS_VERSION);
    log::info!(
        "Bootloader v{}.{}.{}",
        boot_info.version_major,
        boot_info.version_minor,
        boot_info.version_patch
    );
    if let Some(fb_info) = graphics::get_global_framebuffer().map(|fb| fb.info()) {
        log::info!(
            "Framebuffer size:{}x{}x{} format:{:?}",
            fb_info.horizontal_resolution,
            fb_info.vertical_resolution,
            fb_info.bytes_per_pixel,
            fb_info.pixel_format
        );
        check_framebuffer_size(fb_info).unwrap();
    }

    let phys_offset = boot_info
        .physical_memory_offset
        .into_option()
        .ok_or(KernelInitError::PhysicalMemoryNotMapped)
        .unwrap();
    log::info!("Loading GDT");
    userspace::init_gdt();
    log::info!("Loading IDT");
    idt::init_idt();
    log::info!("Setting up kernel memory");
    memory::init_memory(phys_offset, &boot_info.memory_regions);
    log::info!("Enabling interrupts");
    idt::init_interrupts();

    log::info!("Initializing ATA");
    ata::init();
    let drive_info = get_first_ata_drive().unwrap();
    log::debug!(
        "Found drive {} size:{}KiB",
        drive_info.model,
        drive_info.size_in_kib()
    );
    let user_partition = get_user_partition(drive_info.drive).unwrap();
    log::debug!("  user partition size:{}KiB", user_partition.size_in_kib());
    filesystem::init(user_partition);
    let entry_point = program::load_program("raytrace.elf").unwrap();
    userspace::enter_userspace(entry_point);
}

fn check_framebuffer_size(fb_info: FrameBufferInfo) -> Result<(), KernelInitError> {
    if fb_info.horizontal_resolution == 640
        && fb_info.vertical_resolution == 480
        && fb_info.bytes_per_pixel == 4
    {
        Ok(())
    } else {
        Err(KernelInitError::FramebufferWrongSize)
    }
}

fn get_first_ata_drive() -> Result<ata::DriveInfo, KernelInitError> {
    ata::list()
        .into_iter()
        .next()
        .ok_or(KernelInitError::AtaFailed)
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

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("{}", info);
    hlt_loop();
}

#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    log::error!("alloc failed: {:?}", layout);
    hlt_loop();
}
