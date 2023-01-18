use alloc::vec::Vec;
use core::{
    num::{ParseIntError, TryFromIntError},
    str::Utf8Error,
};
use tar_no_std::TarArchiveRef;

use crate::{Level, Tileset};

#[derive(Debug)]
pub enum LevelLoadError {
    CsvNotUtf8,
    CsvWrongSize,
    CsvInvalidValue(ParseIntError),
    CsvValueOutOfRange,
}

impl From<Utf8Error> for LevelLoadError {
    fn from(_err: Utf8Error) -> Self {
        LevelLoadError::CsvNotUtf8
    }
}
impl From<ParseIntError> for LevelLoadError {
    fn from(err: ParseIntError) -> Self {
        LevelLoadError::CsvInvalidValue(err)
    }
}
impl From<TryFromIntError> for LevelLoadError {
    fn from(_err: TryFromIntError) -> Self {
        LevelLoadError::CsvValueOutOfRange
    }
}

pub struct LevelArchive;

impl LevelArchive {
    pub fn load_csv(
        data: &str,
        width: &mut usize,
        height: &mut usize,
    ) -> Result<Vec<u8>, LevelLoadError> {
        let mut tiles = Vec::new();
        let mut data_height = 0;
        for line in data.split('\n') {
            let mut data_width = 0;
            if line.is_empty() {
                continue;
            }
            for value in line.split(',') {
                let value = value.parse::<i32>()? + 1;
                let value = u8::try_from(value)?;
                tiles.push(value);
                data_width += 1;
            }
            if data_width > 0 {
                if *width > 0 && *width != data_width {
                    return Err(LevelLoadError::CsvWrongSize);
                }
                *width = data_width;
                data_height += 1;
            }
        }
        if *height > 0 && *height != data_height {
            return Err(LevelLoadError::CsvWrongSize);
        }
        *height = data_height;
        Ok(tiles)
    }
    pub fn load(data: &[u8]) -> Result<Level, LevelLoadError> {
        let archive = TarArchiveRef::new(data);
        let mut width = 0;
        let mut height = 0;
        let mut background_tileset = Tileset::default();
        let mut background_tiles = Vec::new();
        let mut foreground_tileset = Tileset::default();
        let mut foreground_tiles = Vec::new();
        for entry in archive.entries() {
            match entry.filename().as_str() {
                "background_tiles.data" => background_tileset = Tileset::from_data(entry.data()),
                "background.csv" => {
                    background_tiles =
                        Self::load_csv(entry.data_as_str()?, &mut width, &mut height)?
                }
                "foreground_tiles.data" => foreground_tileset = Tileset::from_data(entry.data()),
                "foreground.csv" => {
                    foreground_tiles =
                        Self::load_csv(entry.data_as_str()?, &mut width, &mut height)?
                }
                _ => (),
            }
        }
        Ok(Level {
            width,
            height,
            scroll: (0, 0),
            background_color: 0xffff9494, // TODO
            background_tileset,
            background_tiles,
            foreground_tileset,
            foreground_tiles,
        })
    }
}
