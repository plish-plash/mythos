use alloc::vec::Vec;
use level::{Level, Object, ObjectDraw};

use crate::graphics::{Framebuffer, GraphicsContext, Image, ImageFormat, LevelRenderer};

pub(crate) static mut WAIT_FRAME: bool = false;

#[derive(Clone, Copy)]
struct LevelId(usize);

#[derive(Clone, Copy)]
struct ObjectId(LevelId, level::ObjectId);

struct Game {
    renderer: LevelRenderer,
    levels: Vec<Option<Level>>,
    player: Option<ObjectId>,
}

impl Game {
    fn new(context: &GraphicsContext, framebuffer: &Framebuffer) -> Self {
        let tile_size = 16 * context.image_scale();
        let foreground_tiles = Image {
            width: 160,
            height: 16,
            format: ImageFormat::Rgba,
            data: include_bytes!("../../assets/foreground_tiles.data"),
        };
        let player_image = Image {
            width: 112,
            height: 16,
            format: ImageFormat::Rgba,
            data: include_bytes!("../../assets/mario.data"),
        };
        let mut renderer = LevelRenderer::new(context, framebuffer, tile_size, &foreground_tiles);
        renderer.add_object_image(context, &player_image);
        Game {
            renderer,
            levels: Vec::new(),
            player: None,
        }
    }
    fn add_level(&mut self, level: Level) -> LevelId {
        for (index, slot) in self.levels.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(level);
                return LevelId(index);
            }
        }
        let index = self.levels.len();
        self.levels.push(Some(level));
        LevelId(index)
    }
    fn active_level(&self) -> Option<LevelId> {
        self.player.map(|id| id.0)
    }
    fn set_active_level(&mut self, id: LevelId) {
        if let Some(player) = self.player {
            self.remove_object(player);
        }
        if let Some(Some(level)) = self.levels.get_mut(id.0) {
            let player_obj = Object {
                kind: "player",
                x: 64.0,
                y: 64.0,
                width: 32,
                height: 32,
                draw: ObjectDraw::Image(0, 0),
            };
            let player_id = level.add_object(player_obj);
            self.player = Some(ObjectId(id, player_id));
        }
    }

    fn add_object(&mut self, level_id: LevelId, object: level::Object) -> Option<ObjectId> {
        if let Some(Some(level)) = self.levels.get_mut(level_id.0) {
            let id = level.add_object(object);
            Some(ObjectId(level_id, id))
        } else {
            None
        }
    }
    fn remove_object(&mut self, id: ObjectId) -> bool {
        if let Some(Some(level)) = self.levels.get_mut(id.0 .0) {
            level.remove_object(id.1)
        } else {
            false
        }
    }

    fn wait_for_next_frame(&self) {
        unsafe {
            WAIT_FRAME = true;
            while WAIT_FRAME {
                x86_64::instructions::hlt();
            }
        }
    }
    fn update(&mut self, context: &GraphicsContext) {
        if let Some(player) = self.player {
            if let Some(Some(level)) = self.levels.get_mut(player.0 .0) {
                let player_obj = level.get_object(player.1).expect("player removed");
                player_obj.x += 1.0;

                self.renderer.draw_level(context, level);
            } else {
                self.player = None;
            }
        }
    }
    fn run(&mut self, context: &GraphicsContext, framebuffer: &mut Framebuffer) -> ! {
        loop {
            self.wait_for_next_frame();
            self.update(context);
            context.write(self.renderer.texture(), framebuffer, 0);
        }
    }
}

pub fn run_game(context: &GraphicsContext, framebuffer: &mut Framebuffer) -> ! {
    let mut game = Game::new(context, framebuffer);
    let level = Level::load(include_bytes!("../../assets/launcher.level")).unwrap();
    let level = game.add_level(level);
    game.set_active_level(level);
    game.run(context, framebuffer);
}
