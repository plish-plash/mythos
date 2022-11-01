# GenOS

A simple, pure-Rust operating system for experimenting with procedural generation.

Relies heavily on the [`bootloader`](https://github.com/rust-osdev/bootloader) crate to setup the environment.

## Features

- Monolithic kernel
- Pretty bitmap font
- Exception and interrupt handling
- Heap allocations with [`linked-list-allocator`](https://github.com/rust-osdev/linked-list-allocator)
- Harddrive access with ATA, FAT32 filesystem
- Userspace ELF programs
  - Switching to ring 3 (userspace), context switching between programs
  - Separate virtual memory for each program, kernel memory is always mapped
  - Syscalls (jumping to kernel code from userspace)

## Structure

- Root crate is the kernel.
- `os-runner` is a binary that combines the bootloader, the kernel, and the user partition to create a bootable disk image.
- `drivers` contain libraries used by the kernel for interfacing with hardware.
- `libraries` contain other libraries, notably a custom std for userspace.
- `programs` contains userspace programs.

## Build Commands

- Run **`./programs/build_user_partition.sh`** to build the programs and create a user partition (requires mtools).
- To build the kernel, run **`cargo kbuild`**.
- To create a bootable disk image with the kernel and user data, run **`cargo kimage`**. This will invoke the `os-runner` sub-crate to create the disk image.
- To create the disk image and run the OS in QEMU, run **`cargo krun`**.
