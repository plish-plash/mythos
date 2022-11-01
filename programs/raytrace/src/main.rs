#![no_main]
#![no_std]

std::entry_point!(main);

fn main() {
    std::test_syscall();
    panic!("userspace panic!");
}
