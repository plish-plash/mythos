#![no_std]
extern crate alloc;

mod archive;

use alloc::{string::String, vec::Vec};

pub use archive::LevelLoadError;

pub enum ObjectDraw {
    Hidden,
    Text(String),
    Image(usize, u32),
}

pub struct Object {
    pub kind: &'static str,
    pub x: f32,
    pub y: f32,
    pub width: u32,
    pub height: u32,
    pub draw: ObjectDraw,
}

impl Object {
    pub fn pixel_x(&self) -> i32 {
        self.x as i32
    }
    pub fn pixel_y(&self) -> i32 {
        self.y as i32
    }
}

#[derive(Clone, Copy)]
pub struct ObjectId(usize);

pub struct Level {
    width: usize,
    height: usize,
    scroll: (i32, i32),
    background_color: u32,
    background_tiles: Vec<u8>,
    foreground_tiles: Vec<u8>,
    objects: Vec<Option<Object>>,
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

    pub fn get_object(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.objects.get_mut(id.0).and_then(|obj| obj.as_mut())
    }
    pub fn add_object(&mut self, object: Object) -> ObjectId {
        for (index, slot) in self.objects.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(object);
                return ObjectId(index);
            }
        }
        let index = self.objects.len();
        self.objects.push(Some(object));
        ObjectId(index)
    }
    pub fn remove_object(&mut self, id: ObjectId) -> bool {
        if let Some(slot) = self.objects.get_mut(id.0) {
            if slot.is_some() {
                *slot = None;
                return true;
            }
        }
        false
    }
    pub fn objects(&self) -> impl Iterator<Item = &Object> {
        self.objects.iter().filter_map(|obj| obj.as_ref())
    }
}
