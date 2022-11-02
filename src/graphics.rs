use bootloader::boot_info::{self, PixelFormat};
use uniquelock::{UniqueGuard, UniqueLock, UniqueOnce};

pub struct FontData<'a> {
    pub buffer: &'a [u8],
    pub width: usize,
    pub char_size: (usize, usize),
}

pub struct FrameBuffer(&'static mut boot_info::FrameBuffer);

impl FrameBuffer {
    #[inline(always)]
    fn set_pixel_color(&mut self, idx: usize, color: u32) {
        let idx = idx * self.0.info().bytes_per_pixel;
        let buffer = self.0.buffer_mut();
        let buffer = &mut buffer[idx] as *mut u8 as *mut u32;
        unsafe {
            *buffer = color;
        }
    }

    pub fn info(&self) -> boot_info::FrameBufferInfo {
        self.0.info()
    }
    pub fn pack_color(&self, r: u8, g: u8, b: u8) -> u32 {
        match self.0.info().pixel_format {
            PixelFormat::RGB => (r as u32) | ((g as u32) << 8) | ((b as u32) << 16),
            PixelFormat::BGR => (b as u32) | ((g as u32) << 8) | ((r as u32) << 16),
            PixelFormat::U8 => r as u32,
            _ => unimplemented!(),
        }
    }

    pub fn put_pixel(&mut self, x: usize, y: usize, color: u32) {
        let idx = x + (y * self.0.info().stride);
        self.set_pixel_color(idx, color);
    }
    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u32) {
        let stride = self.0.info().stride;
        let mut idx = x + (y * stride);
        for _y_i in y..(y + h) {
            for _x_i in x..(x + w) {
                self.set_pixel_color(idx, color);
                idx += 1;
            }
            idx += stride - w;
        }
    }
    pub fn draw_font_char(
        &mut self,
        x: usize,
        y: usize,
        font: &FontData,
        source_x: usize,
        source_y: usize,
        source_scale: usize,
        fg_color: u32,
        bg_color: u32,
    ) {
        let stride = self.0.info().stride;
        let mut source_idx =
            (source_x * font.char_size.0) + (source_y * font.char_size.1 * font.width);
        let mut source_skip_x = 1;
        let mut source_skip_y = 1;
        let mut dest_idx = x + (y * stride);
        let w = font.char_size.0 * source_scale;
        let h = font.char_size.1 * source_scale;

        for _y_i in y..(y + h) {
            for _x_i in x..(x + w) {
                if font.buffer[source_idx] > 0 {
                    self.set_pixel_color(dest_idx, fg_color);
                } else {
                    self.set_pixel_color(dest_idx, bg_color);
                }
                if source_skip_x < source_scale {
                    source_skip_x += 1;
                } else {
                    source_skip_x = 1;
                    source_idx += 1;
                }
                dest_idx += 1;
            }
            if source_skip_y < source_scale {
                source_skip_y += 1;
                source_idx -= font.char_size.0;
            } else {
                source_skip_y = 1;
                source_idx += font.width - font.char_size.0;
            }
            dest_idx += stride - w;
        }
    }
}

static GLOBAL_FRAMEBUFFER: UniqueOnce<UniqueLock<FrameBuffer>> = UniqueOnce::new();

pub fn set_global_framebuffer(framebuffer: &'static mut boot_info::FrameBuffer) {
    GLOBAL_FRAMEBUFFER
        .call_once(|| {
            assert_eq!(framebuffer.info().bytes_per_pixel, 4);
            UniqueLock::new("framebuffer", FrameBuffer(framebuffer))
        })
        .expect("set_global_framebuffer called twice");
}

pub fn get_global_framebuffer() -> Option<UniqueGuard<'static, FrameBuffer>> {
    GLOBAL_FRAMEBUFFER
        .get()
        .ok()
        .and_then(|mtx| mtx.lock().ok())
}
