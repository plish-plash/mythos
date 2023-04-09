This folder contains files to build a custom Rust compiler and std for the operating system. This involves downloading the Rust source code and applying some patches, and generally takes much longer than building the actual operating system.

## Clone the Rust source code
Do NOT do this inside this project, it's a Rust workspace which will mess with the build.
```bash
git clone --depth 1 https://github.com/rust-lang/rust.git mythos-rust
cd mythos-rust
```

## Add custom std
copy "mythos" to library/std/src/sys/
patch library/std/src/sys/mod.rs
patch library/std/build.rs and library/std/src/sys_common/mod.rs

## Add x86_64-unknown-mythos target to the compiler
copy spec files to compiler/rustc_target/src/spec/
patch compiler/rustc_target/src/spec/mod.rs
patch src/bootstrap/lib.rs

## Configure the build
copy "config.toml" to .

## Build
```bash
./x.py build library
```

## Create local toolchain
```bash
rustup toolchain link mythos ~/.../mythos-rust/build/x86_64-unknown-linux-gnu/stage2
```
...and we're done! Now we can cross-compile any old Rust crate with `cargo +mythos build --target=x86_64-unknown-mythos`.