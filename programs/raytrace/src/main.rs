#![no_main]
#![no_std]

use std::{entry_point, screen, wait_for_confirm};

entry_point!(main);

fn main() {
    screen::create(true).unwrap();
    for y in 0..480 {
        for x in 0..640 {
            let r = if y % 3 == 0 { 255 } else { 0 };
            let g = if y % 3 == 1 { 255 } else { 0 };
            let b = if y % 3 == 2 { 255 } else { 0 };
            screen::set_pixel(x, y, screen::Color::new(r, g, b)).unwrap();
        }
    }
    wait_for_confirm();
}
