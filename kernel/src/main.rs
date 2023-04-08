#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(asm_const)]
#![feature(step_trait)]
#![no_std]
#![no_main]
extern crate alloc;

mod elf_loader;
mod graphics;
mod interrupt;
mod memory;
mod userspace;

use alloc::{format, string::String};
use bootloader_api::{config::Mapping, entry_point, BootInfo, BootloaderConfig};

static OS_NAME: &str = "MariOS";
static OS_VERSION: &str = env!("CARGO_PKG_VERSION");
static mut BOOTLOADER_VERSION: Option<String> = None;

static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::FixedAddress(0xf000_0000_0000));
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // Save the framebuffer info from the bootloader.
    let framebuffer_memory =
        graphics::init_graphics(boot_info.framebuffer.as_mut().expect("no framebuffer"));

    // Configure core hardware.
    userspace::init_gdt();
    interrupt::init_idt();
    memory::init_memory(
        boot_info
            .physical_memory_offset
            .into_option()
            .expect("physical memory not mapped"),
        &boot_info.memory_regions,
    );
    interrupt::init_interrupts();

    // Save bootloader version
    let api_version = boot_info.api_version;
    let bootloader_version = format!(
        "{}.{}.{}",
        api_version.version_major(),
        api_version.version_minor(),
        api_version.version_patch()
    );
    unsafe {
        BOOTLOADER_VERSION = Some(bootloader_version);
    }

    // Allow userspace to directly access the framebuffer memory.
    memory::user_memory_mapper()
        .make_range_user_accessible(framebuffer_memory)
        .unwrap();

    // Start the userspace program, which loads drivers and other programs from the filesystem.
    let ramdisk = unsafe {
        core::slice::from_raw_parts(
            boot_info
                .ramdisk_addr
                .into_option()
                .expect("bootloader did not load ramdisk") as *const u8,
            boot_info.ramdisk_len as usize,
        )
    };
    elf_loader::start_load().unwrap();
    elf_loader::load_bytes(ramdisk).unwrap();
    let (entry_point, _tls_template) = elf_loader::finish_load().unwrap();
    userspace::enter_userspace(entry_point);

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

// fn get_first_ata_drive() -> ata::DriveInfo {
//     ata::list()
//         .unwrap()
//         .into_iter()
//         .next()
//         .expect("no connected drives")
// }

// fn get_user_partition(drive: ata::Drive) -> ata::Partition {
//     let mut mbr_bytes = alloc::vec![0u8; 512];
//     drive.read(&mut mbr_bytes, 0, 1).unwrap();
//     let mbr = mbr::MasterBootRecord::from_bytes(&mbr_bytes).unwrap();
//     if mbr.entries[2].partition_type != mbr::PartitionType::Fat32(0x0c) || !mbr.entries[2].bootable {
//         panic!("invalid filesystem partition");
//     }
//     ata::Partition::new(
//         drive,
//         mbr.entries[2].logical_block_address as usize,
//         mbr.entries[2].sector_count as usize,
//     )
// }

#[macro_export]
macro_rules! fatal_error {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        if let Some(mut framebuffer) = unsafe { $crate::graphics::framebuffer() } {
            let context = $crate::graphics::context();
            let mut error_writer = $crate::graphics::TextWriter::new(&context, &mut framebuffer, 0, 0);
            error_writer.write_fmt(format_args!($($arg)*)).ok();
        }
        loop {
            x86_64::instructions::hlt();
        }
    }}
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    fatal_error!("{}", info);
}

#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    fatal_error!("alloc failed: {:?}", layout);
}
