# Mythos

A simple operating system written in Rust.

Relies heavily on the [`bootloader`](https://github.com/rust-osdev/bootloader) crate to setup the environment.

## Features

- Microkernel
- Pretty bitmap font
- Exception and interrupt handling
- Heap allocations with [`linked-list-allocator`](https://github.com/rust-osdev/linked-list-allocator)
- Hard disk access with ATA, FAT32 filesystem
- Userspace ELF programs

## Structure

- The root crate is a binary that builds the kernel and userspace program and assembles a bootable disk image. The entire operating system can be built with a simple `cargo build` and run in QEMU with `cargo run`.
- `kernel` is the OS itself.
- `libraries` contain libraries used by the kernel.
- `userspace` contains the initial userspace program, loaded as a ramdisk by the bootloader.
- `toolchain` contains code for building a custom Rust toolchain for the operating system. See README.md in that folder for details.
