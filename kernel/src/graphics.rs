use alloc::vec::Vec;
use bootloader_api::info::{self as boot_info, PixelFormat};
use core::fmt::Write;
use level::{Level, Object, ObjectDraw};

#[derive(Clone, Copy, Debug, Default)]
pub struct Point {
    x: i32,
    y: i32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Rect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

pub trait Texture {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn stride(&self) -> usize;
    fn data(&self) -> &[u8];
    fn data_mut(&mut self) -> &mut [u8];
}

pub struct Framebuffer(&'static mut boot_info::FrameBuffer);

impl Framebuffer {
    pub fn make_context(&self) -> GraphicsContext {
        const IMAGE_SCALE: u32 = 2;
        GraphicsContext {
            pixel_format: self.0.info().pixel_format,
            bytes_per_pixel: self.0.info().bytes_per_pixel,
            image_scale: IMAGE_SCALE,
        }
    }
}

static mut FRAMEBUFFER: Option<Framebuffer> = None;

pub fn set_framebuffer(info: &'static mut boot_info::FrameBuffer) {
    unsafe {
        FRAMEBUFFER = Some(Framebuffer(info));
    }
}

pub fn setup_context() -> (GraphicsContext, &'static mut Framebuffer) {
    unsafe {
        if let Some(framebuffer) = FRAMEBUFFER.as_mut() {
            let context = framebuffer.make_context();
            (context, framebuffer)
        } else {
            // Can't do anything useful without a framebuffer, so halt the processor, since that's
            // better than triple faulting.
            crate::hlt_loop();
        }
    }
}

impl Texture for Framebuffer {
    fn width(&self) -> u32 {
        self.0.info().width as u32
    }
    fn height(&self) -> u32 {
        self.0.info().height as u32
    }
    fn stride(&self) -> usize {
        self.0.info().stride
    }
    fn data(&self) -> &[u8] {
        self.0.buffer()
    }
    fn data_mut(&mut self) -> &mut [u8] {
        self.0.buffer_mut()
    }
}

pub struct Buffer<T: AsRef<[u8]> + AsMut<[u8]>> {
    width: u32,
    height: u32,
    data: T,
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Texture for Buffer<T> {
    fn width(&self) -> u32 {
        self.width
    }
    fn height(&self) -> u32 {
        self.height
    }
    fn stride(&self) -> usize {
        self.width as usize
    }
    fn data(&self) -> &[u8] {
        self.data.as_ref()
    }
    fn data_mut(&mut self) -> &mut [u8] {
        self.data.as_mut()
    }
}

type VecBuffer = Buffer<Vec<u8>>;

impl Default for VecBuffer {
    fn default() -> Self {
        VecBuffer {
            width: 0,
            height: 0,
            data: Vec::new(),
        }
    }
}

impl VecBuffer {
    fn alloc(context: &GraphicsContext, width: u32, height: u32) -> Self {
        let bytes = (width * height) as usize * context.bytes_per_pixel;
        let mut data = Vec::with_capacity(bytes);
        unsafe {
            data.set_len(bytes);
        }
        Buffer {
            width,
            height,
            data,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ImageFormat {
    Rgba,
    Mask([u8; 3], [u8; 3]),
}

impl ImageFormat {
    fn bytes_per_pixel(&self) -> usize {
        match self {
            ImageFormat::Rgba => 4,
            ImageFormat::Mask(_, _) => 1,
        }
    }
}

pub struct Image<'a> {
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
    pub data: &'a [u8],
}

impl<'a> Image<'a> {
    fn alloc_and_write(&self, context: &GraphicsContext) -> VecBuffer {
        let mut texture = VecBuffer::alloc(
            context,
            self.width * context.image_scale,
            self.height * context.image_scale,
        );
        context.write_image_to_texture(self, &mut texture);
        texture
    }
}

pub struct GraphicsContext {
    pixel_format: PixelFormat,
    bytes_per_pixel: usize,
    image_scale: u32,
}

impl GraphicsContext {
    pub fn image_scale(&self) -> u32 {
        self.image_scale
    }

    fn byte_offset(&self, x: usize, y: usize, texture_stride: usize) -> isize {
        (((y * texture_stride) + x) * self.bytes_per_pixel) as isize
    }
    fn encode_color(&self, r: u8, g: u8, b: u8) -> u32 {
        match self.pixel_format {
            PixelFormat::Rgb => (r as u32) | ((g as u32) << 8) | ((b as u32) << 16),
            PixelFormat::Bgr => (b as u32) | ((g as u32) << 8) | ((r as u32) << 16),
            PixelFormat::U8 => r as u32,
            _ => panic!("unknown pixel format"),
        }
    }
    fn get_image_pixel(&self, image: &Image, x: u32, y: u32) -> u32 {
        let bpp = image.format.bytes_per_pixel();
        let idx = ((y * image.width) + x) as usize * bpp;
        let pixel = &image.data[idx..idx + bpp];
        match image.format {
            ImageFormat::Rgba => self.encode_color(pixel[0], pixel[1], pixel[2]),
            ImageFormat::Mask(fg, bg) => {
                if pixel[0] > 0 {
                    self.encode_color(fg[0], fg[1], fg[2])
                } else {
                    self.encode_color(bg[0], bg[1], bg[2])
                }
            }
        }
    }

    pub fn clear<T: Texture>(&self, texture: &mut T) {
        let data = texture.data_mut();
        unsafe {
            core::ptr::write_bytes(data.as_mut_ptr(), 0, data.len());
        }
    }
    pub fn set_pixel<T: Texture>(&self, texture: &mut T, x: u32, y: u32, color: u32) {
        let src = &color as *const u32 as *const u8;
        unsafe {
            let dst = texture.data_mut().as_mut_ptr().offset(self.byte_offset(
                x as usize,
                y as usize,
                texture.stride(),
            ));
            core::ptr::copy_nonoverlapping(src, dst, self.bytes_per_pixel);
        }
    }
    pub fn write<S: Texture, D: Texture>(&self, source: &S, dest: &mut D, dest_offset: usize) {
        if dest.width() < source.width() || dest.height() < source.height() {
            return;
        }
        let source = source.data();
        unsafe {
            core::ptr::copy_nonoverlapping(
                source.as_ptr(),
                dest.data_mut().as_mut_ptr().offset(dest_offset as isize),
                source.len(),
            );
        }
    }
    pub fn blit<S: Texture, D: Texture>(
        &self,
        source: &S,
        mut source_rect: Rect,
        dest: &mut D,
        mut dest_point: Point,
    ) {
        if dest_point.x < 0 {
            source_rect.x += -dest_point.x;
            source_rect.width = source_rect
                .width
                .checked_sub(-dest_point.x as u32)
                .unwrap_or(0);
            dest_point.x = 0;
        }
        if dest_point.y < 0 {
            source_rect.y += -dest_point.y;
            source_rect.height = source_rect
                .height
                .checked_sub(-dest_point.y as u32)
                .unwrap_or(0);
            dest_point.y = 0;
        }
        // TODO also clamp dest_point on the positive side
        if source_rect.x < 0
            || source_rect.y < 0
            || source_rect.width == 0
            || source_rect.height == 0
        {
            return;
        }

        let row_bytes = source_rect.width as usize * self.bytes_per_pixel;
        unsafe {
            let mut source_ptr = source.data().as_ptr().offset(self.byte_offset(
                source_rect.x as usize,
                source_rect.y as usize,
                source.stride(),
            ));
            let mut dest_ptr = dest.data_mut().as_mut_ptr().offset(self.byte_offset(
                dest_point.x as usize,
                dest_point.y as usize,
                dest.stride(),
            ));
            for _row in 0..source_rect.height {
                core::ptr::copy_nonoverlapping(source_ptr, dest_ptr, row_bytes);
                source_ptr = source_ptr.offset((source.stride() * self.bytes_per_pixel) as isize);
                dest_ptr = dest_ptr.offset((dest.stride() * self.bytes_per_pixel) as isize);
            }
        }
    }

    pub fn write_image_to_texture<T: Texture>(&self, source: &Image, dest: &mut T) {
        if dest.width() < source.width * self.image_scale
            || dest.height() < source.height * self.image_scale
        {
            panic!("texture too small");
        }
        for y in 0..source.height {
            for x in 0..source.width {
                let color = self.get_image_pixel(source, x, y);
                for bx in 0..self.image_scale {
                    for by in 0..self.image_scale {
                        self.set_pixel(
                            dest,
                            (x * self.image_scale) + bx,
                            (y * self.image_scale) + by,
                            color,
                        );
                    }
                }
            }
        }
    }
}

const FONT_TEXTURE_SIZE: usize = 128 * 2 * 64 * 2 * 4;

struct Font {
    texture: Buffer<[u8; FONT_TEXTURE_SIZE]>,
    char_width: u32,
    char_height: u32,
}

impl Font {
    fn load(&mut self, context: &GraphicsContext, image: &Image) {
        context.write_image_to_texture(image, &mut self.texture);
    }
    fn draw_char<T: Texture>(
        &self,
        context: &GraphicsContext,
        char_index: u32,
        dest: &mut T,
        dest_point: Point,
    ) {
        let cols = self.texture.width() / self.char_width;
        let x = ((char_index % cols) * self.char_width) as i32;
        let y = ((char_index / cols) * self.char_height) as i32;
        let source_rect = Rect {
            x,
            y,
            width: self.char_width,
            height: self.char_height,
        };
        context.blit(&self.texture, source_rect, dest, dest_point);
    }
}

static mut SYSTEM_FONT: Font = Font {
    texture: Buffer {
        width: 128 * 2,
        height: 64 * 2,
        data: [0; FONT_TEXTURE_SIZE],
    },
    char_width: 7 * 2,
    char_height: 9 * 2,
};

pub fn load_system_font(context: &GraphicsContext) {
    let image = Image {
        width: 128,
        height: 64,
        format: ImageFormat::Mask([255, 255, 255], [0, 0, 0]),
        data: include_bytes!("font.data"),
    };
    unsafe {
        SYSTEM_FONT.load(context, &image);
    }
}

pub struct TextWriter<'a, T: Texture> {
    context: &'a GraphicsContext,
    texture: &'a mut T,
    start_x: i32,
    wrap_x: i32,
    x: i32,
    y: i32,
}

impl<'a, T: Texture> TextWriter<'a, T> {
    pub fn new(context: &'a GraphicsContext, texture: &'a mut T, x: i32, y: i32) -> Self {
        let wrap_x = texture.width() as i32;
        TextWriter {
            context,
            texture,
            start_x: x,
            wrap_x,
            x,
            y,
        }
    }
    pub fn center_x(&mut self, width: u32, chars: usize) {
        let string_width = chars as u32 * unsafe { SYSTEM_FONT.char_width };
        self.start_x = (width as i32 / 2) - (string_width as i32 / 2);
        self.x = self.start_x;
    }

    fn write_byte(&mut self, byte: u8) {
        let char_width = unsafe { SYSTEM_FONT.char_width as i32 };
        let char_height = unsafe { SYSTEM_FONT.char_height as i32 };
        match byte {
            b'\n' => {
                self.x = self.start_x;
                self.y += char_height;
            }
            byte => {
                if self.x + char_width >= self.wrap_x {
                    self.x = self.start_x;
                    self.y += char_height;
                }
                unsafe {
                    SYSTEM_FONT.draw_char(
                        self.context,
                        (byte - 0x20) as u32,
                        self.texture,
                        Point {
                            x: self.x,
                            y: self.y,
                        },
                    );
                }
                self.x += char_width;
            }
        }
    }
}

impl<'a, T: Texture> Write for TextWriter<'a, T> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            match byte {
                // printable ASCII byte or newline
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // not part of printable ASCII range, print as '?'
                _ => self.write_byte(b'?'),
            }
        }
        Ok(())
    }
}

pub struct LevelRenderer {
    texture: VecBuffer,
    tile_size: u32,
    background_color: VecBuffer,
    background_tiles: VecBuffer,
    foreground_tiles: VecBuffer,
    object_images: Vec<VecBuffer>,
}

impl LevelRenderer {
    pub fn new(
        context: &GraphicsContext,
        framebuffer: &Framebuffer,
        tile_size: u32,
        foreground_tiles: &Image,
    ) -> Self {
        let texture = VecBuffer::alloc(context, framebuffer.stride() as u32, framebuffer.height());
        let mut background_color = VecBuffer::alloc(context, framebuffer.stride() as u32, 1);
        let color = context.encode_color(0x94, 0x94, 0xff);
        for x in 0..background_color.width() {
            context.set_pixel(&mut background_color, x, 0, color);
        }
        let background_tiles = VecBuffer::default();
        let foreground_tiles = foreground_tiles.alloc_and_write(context);
        LevelRenderer {
            texture,
            tile_size,
            background_color,
            background_tiles,
            foreground_tiles,
            object_images: Vec::new(),
        }
    }
    pub fn add_object_image(&mut self, context: &GraphicsContext, image: &Image) -> usize {
        let index = self.object_images.len();
        self.object_images.push(image.alloc_and_write(context));
        index
    }
    pub fn texture(&self) -> &VecBuffer {
        &self.texture
    }

    fn draw_tile(&mut self, context: &GraphicsContext, level: &Level, x: u32, y: u32) {
        let dest_x = (x * self.tile_size) as i32 + level.scroll_x();
        let dest_y = (y * self.tile_size) as i32 + level.scroll_y();
        if dest_x < 0
            || dest_x >= self.texture.width() as i32
            || dest_y < 0
            || dest_y >= self.texture.height() as i32
        {
            return;
        }
        let tile = level.get_foreground_tile(x, y) as u32;
        if tile > 0 {
            let source_rect = Rect {
                x: ((tile - 1) * self.tile_size) as i32,
                y: 0,
                width: self.tile_size,
                height: self.tile_size,
            };
            context.blit(
                &self.foreground_tiles,
                source_rect,
                &mut self.texture,
                Point {
                    x: dest_x,
                    y: dest_y,
                },
            );
            return;
        }
        let tile = level.get_background_tile(x, y) as u32;
        if tile > 0 {
            let source_rect = Rect {
                x: ((tile - 1) * self.tile_size) as i32,
                y: 0,
                width: self.tile_size,
                height: self.tile_size,
            };
            context.blit(
                &self.background_tiles,
                source_rect,
                &mut self.texture,
                Point {
                    x: dest_x,
                    y: dest_y,
                },
            );
        }
    }
    fn draw_object(&mut self, context: &GraphicsContext, object: &Object) {
        match object.draw {
            ObjectDraw::Hidden => (),
            ObjectDraw::Text(_) => todo!(),
            ObjectDraw::Image(index, frame) => {
                let image = &self.object_images[index];
                let source_rect = Rect {
                    x: (frame * object.width) as i32,
                    y: 0,
                    width: object.width,
                    height: object.height,
                };
                let dest_point = Point {
                    x: object.pixel_x(),
                    y: object.pixel_y(),
                };
                context.blit(image, source_rect, &mut self.texture, dest_point);
            }
        }
    }
    pub fn draw_level(&mut self, context: &GraphicsContext, level: &Level) {
        let stride = self.texture.stride() * context.bytes_per_pixel;
        for y in 0..self.texture.height() {
            context.write(
                &self.background_color,
                &mut self.texture,
                (y as usize) * stride,
            );
        }
        for y in 0..level.height() {
            for x in 0..level.width() {
                self.draw_tile(context, level, x as u32, y as u32);
            }
        }
        for object in level.objects() {
            self.draw_object(context, object);
        }
    }
}
