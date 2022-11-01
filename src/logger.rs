use core::fmt::Write;
use log::{Record, Level, Metadata};
use crate::{graphics, screen::{Screen, TextScreen, Palette, PaletteColor}};

static KERNEL_TEXT_SCREEN: spin::Mutex<TextScreen> = spin::Mutex::new(TextScreen::kernel_new());

trait IntoColor {
    fn into_color(self) -> PaletteColor;
}

impl IntoColor for Level {
    fn into_color(self) -> PaletteColor {
        PaletteColor::new(self as u8)
    }
}

struct TextWriter<'a> {
    x_position: usize,
    color: PaletteColor,
    screen: spin::MutexGuard<'a, TextScreen>,
}

impl<'a> TextWriter<'a> {
    fn lock_kernel_screen(log_level: Level) -> TextWriter<'static> {
        TextWriter { x_position: 0, color: log_level.into_color(), screen: KERNEL_TEXT_SCREEN.lock() }
    }
    fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                self.scroll_up();
                self.x_position = 0;
            },
            byte => {
                if self.x_position >= TextScreen::WIDTH {
                    self.scroll_up();
                    self.x_position = 0;
                }
                self.screen.set_char(self.x_position, TextScreen::HEIGHT - 1, byte - 0x20, self.color);
                self.x_position += 1;
            }
        }
    }
    fn scroll_up(&mut self) {
        self.screen.scroll_up(1);
    }
}

impl<'a> Write for TextWriter<'a> {
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

struct KernelLogger;

impl log::Log for KernelLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let mut writer = TextWriter::lock_kernel_screen(record.level());
            writer.scroll_up();
            write!(writer, "{}", record.args()).unwrap();
            if record.level() == Level::Error {
                writer.screen.set_active(true);
            }
        }
    }
    fn flush(&self) {}
}

static LOGGER: KernelLogger = KernelLogger;

pub fn init() -> Result<(), log::SetLoggerError> {
    // Setup screen
    let palette = graphics::get_global_framebuffer().map(|fb| {
        let mut palette = Palette::new();
        palette.set_color(Level::Trace.into_color(), fb.pack_color(128, 128, 255));
        palette.set_color(Level::Debug.into_color(), fb.pack_color(192, 192, 192));
        palette.set_color(Level::Info.into_color(),  fb.pack_color(255, 255, 255));
        palette.set_color(Level::Warn.into_color(),  fb.pack_color(255, 128, 0));
        palette.set_color(Level::Error.into_color(), fb.pack_color(255, 0, 0));
        palette
    });
    if let Some(palette) = palette {
        let mut screen = KERNEL_TEXT_SCREEN.lock();
        screen.set_palette(palette);
        screen.set_active(true);
    }
    // Setup logger
    log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Trace))
}

pub fn show_kernel_screen() {
    KERNEL_TEXT_SCREEN.lock().set_active(true);
}
