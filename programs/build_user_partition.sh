#!/bin/bash

PROGRAMS=raytrace
PROGRAM_DIR=$(dirname "$0")

BUILD_CMD="cargo build --target ../../x86_64-user.json -Zbuild-std=core,alloc -Zbuild-std-features=compiler-builtins-mem --release"
for prog in "$PROGRAMS"; do (cd "$PROGRAM_DIR/$prog" && $BUILD_CMD); done

FS_IMAGE=$PROGRAM_DIR/../target/user_partition.img
[ -f "$FS_IMAGE" ] && rm "$FS_IMAGE"
echo Creating FAT32 filesystem
dd if=/dev/zero of="$FS_IMAGE" bs=1M count=2
mformat -F -i "$FS_IMAGE" ::
mmd -i "$FS_IMAGE" ::/programs
for prog in "$PROGRAMS"; do (mcopy -i "$FS_IMAGE" target/x86_64-user/release/$prog ::/programs/$prog.elf); done
