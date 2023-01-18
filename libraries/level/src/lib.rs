#![no_std]
extern crate alloc;

mod archive;

use alloc::vec::Vec;

pub use archive::LevelLoadError;

pub trait BlitSource {
    fn stride(&self) -> usize;
    fn blit_width(&self) -> u32;
    fn blit_height(&self) -> u32;
    fn index(&self, x: u32, y: u32) -> usize {
        x as usize + (y as usize * self.stride())
    }
    fn get_pixel(&self, index: usize) -> u32;
}

#[derive(Default)]
pub struct Tileset(Vec<u32>);

impl Tileset {
    pub fn from_data(data: &[u8]) -> Self {
        Tileset(
            data.chunks_exact(4)
                .map(|value| u32::from_ne_bytes(value.try_into().unwrap()))
                .collect(),
        )
    }
}

impl BlitSource for Tileset {
    fn stride(&self) -> usize {
        self.0.len() / 16
    }
    fn blit_width(&self) -> u32 {
        16
    }
    fn blit_height(&self) -> u32 {
        16
    }
    fn index(&self, x: u32, _y: u32) -> usize {
        (x * 16) as usize
    }
    fn get_pixel(&self, index: usize) -> u32 {
        self.0[index]
    }
}

pub struct Level {
    width: usize,
    height: usize,
    scroll: (i32, i32),
    background_color: u32,
    background_tileset: Tileset,
    background_tiles: Vec<u8>,
    foreground_tileset: Tileset,
    foreground_tiles: Vec<u8>,
}

impl Level {
    pub fn load(data: &[u8]) -> Result<Self, LevelLoadError> {
        archive::LevelArchive::load(data)
    }

    pub fn width(&self) -> usize {
        self.width
    }
    pub fn height(&self) -> usize {
        self.height
    }
    pub fn scroll_x(&self) -> i32 {
        self.scroll.0
    }
    pub fn scroll_y(&self) -> i32 {
        self.scroll.1
    }
    pub fn background_color(&self) -> u32 {
        self.background_color
    }
    pub fn background_tileset(&self) -> &Tileset {
        &self.background_tileset
    }
    pub fn foreground_tileset(&self) -> &Tileset {
        &self.foreground_tileset
    }

    fn get_index(&self, x: u32, y: u32) -> usize {
        x as usize + (y as usize * self.width)
    }
    pub fn get_background_tile(&self, x: u32, y: u32) -> u8 {
        self.background_tiles
            .get(self.get_index(x, y))
            .map(|t| *t)
            .unwrap_or_default()
    }
    pub fn set_background_tile(&mut self, x: u32, y: u32, tile: u8) {
        let idx = self.get_index(x, y);
        self.background_tiles[idx] = tile;
    }
    pub fn get_foreground_tile(&self, x: u32, y: u32) -> u8 {
        self.foreground_tiles
            .get(self.get_index(x, y))
            .map(|t| *t)
            .unwrap_or_default()
    }
    pub fn set_foreground_tile(&mut self, x: u32, y: u32, tile: u8) {
        let idx = self.get_index(x, y);
        self.foreground_tiles[idx] = tile;
    }
}
