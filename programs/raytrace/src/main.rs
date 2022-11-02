#![no_main]
#![no_std]

use std::{entry_point, screen, wait_for_confirm};

entry_point!(main);

fn main() {
    screen::create(true).unwrap();
    for y in 0..480 {
        let t = y as f32 / 480.0;
        for x in 0..640 {
            let col = (t * 255.0) as u8;
            screen::set_pixel(x, y, screen::Color::new(col, col, 255)).unwrap();
        }
    }
    wait_for_confirm();
}
