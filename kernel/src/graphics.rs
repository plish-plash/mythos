use crate::memory::VirtMemRange;

pub use kernel_common::graphics::*;

static mut FRAMEBUFFER: Option<FrameBuffer> = None;
static mut GRAPHICS_CONTEXT: GraphicsContext = GraphicsContext::const_default();

pub fn init_graphics(framebuffer: &'static mut bootloader_api::info::FrameBuffer) -> VirtMemRange {
    let data = framebuffer.buffer_mut();
    let fb_memory = VirtMemRange::new(data.as_ptr() as u64, data.len());
    data.fill(0);
    let context = GraphicsContext::from_framebuffer(framebuffer);
    let buffer = FrameBuffer::from_framebuffer(framebuffer);
    load_system_font(&context, [255, 64, 64]);
    unsafe {
        FRAMEBUFFER = Some(buffer);
        GRAPHICS_CONTEXT = context;
    }
    fb_memory
}

pub fn context() -> GraphicsContext {
    unsafe { GRAPHICS_CONTEXT.clone() }
}

// UNSAFE: this function will create multiple mutable references to the framebuffer, use with care!
pub unsafe fn framebuffer() -> Option<FrameBuffer> {
    let mut framebuffer = None;
    core::ptr::copy_nonoverlapping(&FRAMEBUFFER as *const _, &mut framebuffer as *mut _, 1);
    framebuffer
}
