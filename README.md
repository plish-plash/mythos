# MariOS

A toy operating system that is controlled by playing Super Mario Bros. Written entirely in Rust.

Relies heavily on the [`bootloader`](https://github.com/rust-osdev/bootloader) crate to setup the environment.

## Features

- Monolithic kernel
- Pretty bitmap font
- Exception and interrupt handling
- Heap allocations with [`linked-list-allocator`](https://github.com/rust-osdev/linked-list-allocator)
- Hard disk access with ATA, FAT32 filesystem
- Userspace ELF programs
  - Switching to ring 3 (userspace), context switching between programs
  - Separate virtual memory for each program, kernel memory is always mapped
  - Syscalls (jumping to kernel code from userspace)

## Structure

- Root crate is a binary that combines the bootloader, the kernel, and the user partition to create a bootable disk image.
- `kernel` is the OS itself.
- `drivers` contain libraries used by the kernel for interfacing with hardware.
- `libraries` contain other libraries, notably a custom std for userspace.
- `programs` contains userspace programs.

## Build Commands

- Run **`./programs/build_user_partition.sh`** to build the programs and create a user partition (requires mtools).
- Run **`cargo build`** to create a bootable disk image, and **`cargo run`** to run it in QEMU.
