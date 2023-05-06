# Mythos Toolchain
This folder contains files to build a custom Rust compiler and standard library for the operating system. This involves downloading the Rust source code and applying some patches, and generally takes much longer than building the kernel.

Also see https://wiki.osdev.org/Porting_Rust_standard_library.

### Clone the Rust source code
Do NOT do this inside this project, it's a Rust workspace which will mess with the build.
```bash
git clone --depth 1 https://github.com/rust-lang/rust.git mythos-rust
cd mythos-rust
```

### Add standard library
Copy the mythos folder to library/std/src/sys/. This is the actual implementation of the standard library.

Apply library.patch to the Rust source. This does a few things:
* Rust will normally use libc for some core functions, most importantly memcpy. We don't have libc, and instead want the compiler to provide implementations. This can be done with a simple feature flag. (library/std/Cargo.toml)
* Add "mythos" to the list of targets that don't have special requirements. (library/std/build.rs)
* Use the Mythos std implementation when building for Mythos. (library/std/src/sys/mod.rs)
* Finally, the normal std::net won't compile for Mythos yet, so instead of trying, just provide a similar API of shim functions. (library/std/src/sys_common/mod.rs)

### Add compiler target
Copy the spec folder to compiler/rustc_target/src/ (merging with the spec folder already there). This provides the x86_64-unknown-mythos target for the compiler.

Apply compiler.patch to the Rust source.
* Add x86_64-unknown-mythos to the compiler's built-in targets. (compiler/rustc_target/src/spec/mod.rs)
* The bootstrap compiler won't know about Mythos, and will error when it sees `#[cfg(target_os = "mythos")]` in the standard library. Luckily there's a setting for this exact situation, to explicitly allow the name "mythos". (src/bootstrap/lib.rs)

### Build
Copy config.toml to the source root, then run
```bash
./x.py build library
```

### Create local toolchain
```bash
rustup toolchain link mythos ~/.../mythos-rust/build/x86_64-unknown-linux-gnu/stage2
```
...and we're done! Now we can cross-compile any old Rust crate with `cargo +mythos build --target=x86_64-unknown-mythos`.
