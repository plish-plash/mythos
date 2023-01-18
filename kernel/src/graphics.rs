use bootloader_api::info::{self as boot_info, PixelFormat};
use core::fmt::Write;
use level::Level;
use uniquelock::{UniqueGuard, UniqueLock, UniqueOnce};

pub use level::BlitSource;

struct ColorMask<'a, T: BlitSource> {
    inner: &'a T,
    foreground: u32,
    background: u32,
}

impl<'a, T: BlitSource> BlitSource for ColorMask<'a, T> {
    fn stride(&self) -> usize {
        self.inner.stride()
    }
    fn blit_width(&self) -> u32 {
        self.inner.blit_width()
    }
    fn blit_height(&self) -> u32 {
        self.inner.blit_height()
    }
    fn index(&self, x: u32, y: u32) -> usize {
        self.inner.index(x, y)
    }
    fn get_pixel(&self, index: usize) -> u32 {
        if self.inner.get_pixel(index) > 0 {
            self.foreground
        } else {
            self.background
        }
    }
}

struct FontData<'a> {
    buffer: &'a [u8],
    width: usize,
    char_size: (u32, u32),
}

impl BlitSource for FontData<'_> {
    fn stride(&self) -> usize {
        self.width
    }
    fn blit_width(&self) -> u32 {
        self.char_size.0
    }
    fn blit_height(&self) -> u32 {
        self.char_size.1
    }
    fn index(&self, x: u32, y: u32) -> usize {
        (x * self.char_size.0) as usize + ((y * self.char_size.1) as usize * self.width)
    }
    fn get_pixel(&self, index: usize) -> u32 {
        self.buffer[index] as u32
    }
}

pub struct FrameBuffer(&'static mut boot_info::FrameBuffer);

impl FrameBuffer {
    #[inline(always)]
    fn set_pixel(&mut self, idx: usize, color: u32) {
        let bpp = self.0.info().bytes_per_pixel;
        let buffer = self.0.buffer_mut();
        let dst = &mut buffer[idx * bpp] as *mut u8;
        let src = &color as *const u32 as *const u8;
        unsafe {
            core::ptr::copy_nonoverlapping(src, dst, bpp);
        }
    }
    fn clear(&mut self) {
        self.0.buffer_mut().fill(0);
    }

    pub fn info(&self) -> boot_info::FrameBufferInfo {
        self.0.info()
    }
    pub fn encode_color(&self, r: u8, g: u8, b: u8) -> u32 {
        match self.0.info().pixel_format {
            PixelFormat::Rgb => (r as u32) | ((g as u32) << 8) | ((b as u32) << 16),
            PixelFormat::Bgr => (b as u32) | ((g as u32) << 8) | ((r as u32) << 16),
            PixelFormat::U8 => r as u32,
            _ => panic!("unknown pixel format"),
        }
    }
    pub fn encode_rgba(&self, rgba: u32) -> Option<u32> {
        let r = (rgba & 0xFF) as u8;
        let g = ((rgba >> 8) & 0xFF) as u8;
        let b = ((rgba >> 16) & 0xFF) as u8;
        let a = ((rgba >> 24) & 0xFF) as u8;
        if a > 0 {
            Some(self.encode_color(r, g, b))
        } else {
            None
        }
    }

    pub fn set_pixel_at(&mut self, x: u32, y: u32, color: u32) {
        let idx = x as usize + (y as usize * self.0.info().stride);
        self.set_pixel(idx, color);
    }
    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        let stride = self.0.info().stride;
        let fb_w = self.0.info().width as u32;
        let fb_h = self.0.info().height as u32;
        let mut row_idx = x as usize + (y as usize * stride);
        let mut idx = row_idx;
        for y_i in y..(y + h) {
            if y_i >= fb_h {
                break;
            }
            for x_i in x..(x + w) {
                if x_i >= fb_w {
                    break;
                }
                self.set_pixel(idx, color);
                idx += 1;
            }
            row_idx += stride;
            idx = row_idx;
        }
    }
    pub fn blit<T: BlitSource>(
        &mut self,
        dest_x: u32,
        dest_y: u32,
        source: &T,
        source_x: u32,
        source_y: u32,
        scale: u32,
        rgba: bool,
    ) {
        let stride = self.0.info().stride;
        let fb_w = self.0.info().width as u32;
        let fb_h = self.0.info().height as u32;
        let mut dest_row_idx = dest_x as usize + (dest_y as usize * stride);
        let mut dest_idx = dest_row_idx;
        let mut source_row_idx = source.index(source_x, source_y);
        let mut source_idx = source_row_idx;
        let mut source_skip_x = 1;
        let mut source_skip_y = 1;

        let w = source.blit_width() * scale;
        let h = source.blit_height() * scale;
        for y in dest_y..(dest_y + h) {
            if y >= fb_h {
                break;
            }
            for x in dest_x..(dest_x + w) {
                if x >= fb_w {
                    break;
                }
                let color = source.get_pixel(source_idx);
                if rgba {
                    if let Some(color) = self.encode_rgba(color) {
                        self.set_pixel(dest_idx, color);
                    }
                } else {
                    self.set_pixel(dest_idx, color);
                }
                if source_skip_x < scale {
                    source_skip_x += 1;
                } else {
                    source_skip_x = 1;
                    source_idx += 1;
                }
                dest_idx += 1;
            }
            if source_skip_y < scale {
                source_skip_y += 1;
                source_idx = source_row_idx;
            } else {
                source_skip_y = 1;
                source_row_idx += source.stride();
                source_idx = source_row_idx;
            }
            dest_row_idx += stride;
            dest_idx = dest_row_idx;
        }
    }
}

static GLOBAL_FRAMEBUFFER: UniqueOnce<UniqueLock<FrameBuffer>> = UniqueOnce::new();

pub fn set_global_framebuffer(framebuffer: &'static mut boot_info::FrameBuffer) {
    GLOBAL_FRAMEBUFFER
        .call_once(|| {
            let mut fb = FrameBuffer(framebuffer);
            fb.clear();
            UniqueLock::new("framebuffer", fb)
        })
        .expect("set_global_framebuffer called twice");
}

pub fn get_global_framebuffer() -> Option<UniqueGuard<'static, FrameBuffer>> {
    GLOBAL_FRAMEBUFFER
        .get()
        .ok()
        .and_then(|mtx| mtx.lock().ok())
}

static FONT: FontData = FontData {
    buffer: include_bytes!("font.data"),
    width: 128,
    char_size: (7, 9),
};

pub struct TextWriter {
    start_x: u32,
    wrap_x: u32,
    x: u32,
    y: u32,
    font_source: ColorMask<'static, FontData<'static>>,
}

impl TextWriter {
    const FONT_SCALE: u32 = 2;
    pub fn string_width(chars: usize) -> u32 {
        chars as u32 * FONT.char_size.0 * Self::FONT_SCALE
    }
    pub fn new(x: u32, y: u32, fg_color: u32, bg_color: u32) -> Self {
        let framebuffer = get_global_framebuffer().expect("no framebuffer");
        TextWriter {
            start_x: x,
            wrap_x: framebuffer.info().width as u32,
            x,
            y,
            font_source: ColorMask {
                inner: &FONT,
                foreground: fg_color,
                background: bg_color,
            },
        }
    }

    fn write_byte(&mut self, framebuffer: &mut FrameBuffer, byte: u8) {
        match byte {
            b'\n' => {
                self.x = self.start_x;
                self.y += FONT.char_size.1 * Self::FONT_SCALE;
            }
            byte => {
                if self.x + (FONT.char_size.0 * Self::FONT_SCALE) >= self.wrap_x {
                    self.x = self.start_x;
                    self.y += FONT.char_size.1 * Self::FONT_SCALE;
                }
                let char_idx = (byte - 0x20) as u32;
                let font_cols = FONT.width as u32 / FONT.char_size.0;
                framebuffer.blit(
                    self.x,
                    self.y,
                    &self.font_source,
                    char_idx % font_cols,
                    char_idx / font_cols,
                    Self::FONT_SCALE,
                    false,
                );
                self.x += FONT.char_size.0 * Self::FONT_SCALE;
            }
        }
    }
}

impl Write for TextWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let mut framebuffer = get_global_framebuffer().expect("no framebuffer");
        for byte in s.bytes() {
            match byte {
                // printable ASCII byte or newline
                0x20..=0x7e | b'\n' => self.write_byte(&mut framebuffer, byte),
                // not part of printable ASCII range, print as '?'
                _ => self.write_byte(&mut framebuffer, b'?'),
            }
        }
        Ok(())
    }
}

pub struct LevelRenderer;

impl LevelRenderer {
    const SCALE: u32 = 2;
    pub fn draw_tile(level: &Level, x: u32, y: u32) {
        let background = level.background_tileset();
        let foreground = level.foreground_tileset();
        let dest_x =
            ((x * background.blit_width()) as i32 + level.scroll_x()) * (Self::SCALE as i32);
        let dest_y =
            ((y * background.blit_height()) as i32 + level.scroll_y()) * (Self::SCALE as i32);
        let (width, height) = {
            let framebuffer = get_global_framebuffer().expect("no framebuffer");
            (
                framebuffer.info().width as i32,
                framebuffer.info().height as i32,
            )
        };
        if dest_x < 0 || dest_x >= width || dest_y < 0 || dest_y >= height {
            return;
        }
        let tile = level.get_background_tile(x, y) as u32;
        if tile > 0 {
            let mut framebuffer = get_global_framebuffer().expect("no framebuffer");
            framebuffer.blit(
                dest_x as u32,
                dest_y as u32,
                background,
                tile - 1,
                0,
                Self::SCALE,
                true,
            );
        }
        let tile = level.get_foreground_tile(x, y) as u32;
        if tile > 0 {
            let mut framebuffer = get_global_framebuffer().expect("no framebuffer");
            framebuffer.blit(
                dest_x as u32,
                dest_y as u32,
                foreground,
                tile - 1,
                0,
                Self::SCALE,
                true,
            );
        }
    }
    pub fn draw_level(level: &Level) {
        {
            let mut framebuffer = get_global_framebuffer().expect("no framebuffer");
            let (width, height) = (
                framebuffer.info().width as u32,
                framebuffer.info().height as u32,
            );
            let background_color = framebuffer
                .encode_rgba(level.background_color())
                .unwrap_or_default();
            framebuffer.fill_rect(0, 0, width, height, background_color);
        }
        for y in 0..level.height() {
            for x in 0..level.width() {
                Self::draw_tile(level, x as u32, y as u32);
            }
        }
    }
}
