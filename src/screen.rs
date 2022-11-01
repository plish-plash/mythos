use crate::graphics::*;

pub trait Screen {
    fn set_active(&mut self, active: bool);
    fn draw_full(&self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteColor(u8);

impl PaletteColor {
    pub fn new(idx: u8) -> PaletteColor { PaletteColor(idx) }
}

pub struct Palette {
    colors: [u32; 16],
}

impl Palette {
    pub const fn new() -> Palette {
        Palette { colors: [0; 16] }
    }
    pub fn set_color(&mut self, color: PaletteColor, value: u32) {
        self.colors[color.0 as usize] = value;
    }
}

const COLOR_BLACK: u32 = 0;

static TEXT_SCREEN_FONT: FontData = FontData {
    buffer: include_bytes!("font.data"),
    width: 128,
    char_size: (7, 9),
};

pub struct TextScreen {
    active: bool,
    palette: Palette,
    data: [(u8, u8); Self::WIDTH * Self::HEIGHT],
}

impl TextScreen {
    const FONT_SCALE: usize = 2;
    pub const WIDTH: usize = 45;
    pub const HEIGHT: usize = 26;

    pub const fn kernel_new() -> TextScreen {
        TextScreen {
            active: false,
            palette: Palette::new(),
            data: [(0, 0); Self::WIDTH * Self::HEIGHT],
        }
    }
    pub fn set_palette(&mut self, palette: Palette) {
        self.palette = palette;
    }

    fn index(x: usize, y: usize) -> usize {
        x + (y * Self::WIDTH)
    }
    pub fn set_char(&mut self, x: usize, y: usize, ch: u8, color: PaletteColor) {
        let idx = Self::index(x, y);
        let value = (ch, color.0);
        if self.data[idx] != value {
            self.data[idx] = value;
            if self.active {
                if let Some(mut fb) = get_global_framebuffer() {
                    self.draw_char(&mut fb, x, y, idx);
                }
            }
        }
    }
    pub fn scroll_up(&mut self, lines: usize) {
        for _i in 0..lines {
            for row in 1..Self::HEIGHT {
                for col in 0..Self::WIDTH {
                    let prev = self.data[(row * Self::WIDTH) + col];
                    self.set_char(col, row - 1, prev.0, PaletteColor::new(prev.1));
                }
            }
            for col in 0..Self::WIDTH {
                self.set_char(col, Self::HEIGHT - 1, 0, PaletteColor::new(0));
            }
        }
    }
    fn draw_char(&self, fb: &mut FrameBuffer, col: usize, row: usize, idx: usize) {
        let w = TEXT_SCREEN_FONT.char_size.0 * Self::FONT_SCALE;
        let h = TEXT_SCREEN_FONT.char_size.1 * Self::FONT_SCALE;
        let x = col * w;
        let y = (row * h) + 12;
        let (ch, color) = self.data[idx];
        let fg_color = self.palette.colors[color as usize];
        if ch == 0 {
            fb.fill_rect(x, y, w, h, COLOR_BLACK);
        } else {
            let ch = ch as usize;
            let font_cols = TEXT_SCREEN_FONT.width / TEXT_SCREEN_FONT.char_size.0;
            fb.draw_font_char(x, y, &TEXT_SCREEN_FONT, ch % font_cols, ch / font_cols, Self::FONT_SCALE, fg_color, COLOR_BLACK);
        }
    }
}

impl Screen for TextScreen {
    fn set_active(&mut self, active: bool) {
        if self.active != active {
            self.active = active;
            if active {
                self.draw_full();
            }
        }
    }
    fn draw_full(&self) {
        if let Some(mut fb) = get_global_framebuffer() {
            let mut idx = 0;
            for y in 0..Self::HEIGHT {
                for x in 0..Self::WIDTH {
                    self.draw_char(&mut fb, x, y, idx);
                    idx += 1;
                }
            }
            // The text rectangle doesn't quite fill the screen, so draw black boxes to clear the rest.
            fb.fill_rect(0, 0, 640, 12, COLOR_BLACK);
            fb.fill_rect(640 - 10, 12, 10, 480 - 12, COLOR_BLACK);
        }
    }
}

pub struct ImageScreen {

}

