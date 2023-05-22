use geng::prelude::*;

enum Drag {
    None,
    Detecting { from: vec2<f32>, timer: Timer },
    Dragging { prev_mouse_pos: vec2<f32> },
}

#[derive(Deserialize)]
struct Config {
    pub min_drag_distance: f32,
    pub fov: f32,
    pub drag_start_timer: f64, // TODO: Duration
}

#[derive(geng::asset::Load)]
struct Assets {
    pub ferris_pirate: ugli::Texture,
    pub ground: ugli::Texture,
    pub obstacles: ugli::Texture,
}

struct Game {
    geng: Geng,
    framebuffer_size: vec2<f32>,
    camera: geng::Camera2d,
    drag: Drag,
    config: Config,
    assets: Assets,
}

impl Game {
    pub fn new(geng: &Geng, assets: Assets, config: Config) -> Self {
        Self {
            assets,
            geng: geng.clone(),
            framebuffer_size: vec2::splat(1.0),
            camera: geng::Camera2d {
                center: vec2::ZERO,
                rotation: 0.0,
                fov: config.fov,
            },
            drag: Drag::None,
            config,
        }
    }
}

impl geng::State for Game {
    fn update(&mut self, delta_time: f64) {
        if let Drag::Detecting { from, timer } = &self.drag {
            if timer.elapsed().as_secs_f64() > self.config.drag_start_timer {
                self.drag = Drag::Dragging { prev_mouse_pos: *from };
            }
        }
    }
    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        self.framebuffer_size = framebuffer.size().map(|x| x as f32);
        ugli::clear(framebuffer, Some(Rgba::BLACK), None, None);

        let mut draw_sprite = |texture: &ugli::Texture, pos: vec2<f32>| {
            self.geng.draw2d().draw2d(
                framebuffer,
                &self.camera,
                &draw2d::TexturedQuad::new(
                    Aabb2::point(pos).extend_symmetric(texture.size().map(|x| x as f32) / 2.0),
                    texture,
                ),
            );
        };

        draw_sprite(&self.assets.ground, vec2::ZERO);
        draw_sprite(&self.assets.ferris_pirate, vec2(300.0, -200.0));
        draw_sprite(&self.assets.obstacles, vec2::ZERO);
    }
    fn handle_event(&mut self, event: geng::Event) {
        let world_pos = |screen_pos| {
            self.camera
                .screen_to_world(self.framebuffer_size, screen_pos)
        };
        match event {
            geng::Event::MouseDown { position, .. }
            | geng::Event::TouchStart(geng::Touch { position, .. }) => {
                let pos = position.map(|x| x as f32);
                self.drag = Drag::Detecting {
                    from: pos,
                    timer: Timer::new(),
                };
            }
            geng::Event::MouseMove { position, .. }
            | geng::Event::TouchMove(geng::Touch { position, .. }) => {
                let pos = position.map(|x| x as f32);
                if let Drag::Detecting { from, .. } = self.drag {
                    if (from - pos).len() > self.config.min_drag_distance {
                        self.drag = Drag::Dragging {
                            prev_mouse_pos: from,
                        };
                    }
                }
                if let Drag::Dragging { prev_mouse_pos } = &mut self.drag {
                    self.camera.center += world_pos(*prev_mouse_pos) - world_pos(pos);
                    *prev_mouse_pos = pos;
                }
            }
            geng::Event::MouseUp { position, .. }
            | geng::Event::TouchEnd(geng::Touch { position, .. }) => {
                let pos = position.map(|x| x as f32);
                if let Drag::Detecting { .. } = self.drag {
                    log::info!("Clicked at {pos:?}");
                }
                self.drag = Drag::None;
            }
            _ => {}
        }
    }
}

fn main() {
    logger::init();
    geng::setup_panic_handler();
    let geng = Geng::new("Find Ferris");
    geng.clone().run_loading(async move {
        let config = file::load_detect(run_dir().join("assets").join("config.toml"))
            .await
            .unwrap();
        let assets = geng
            .asset_manager()
            .load(run_dir().join("assets"))
            .await
            .unwrap();
        Game::new(&geng, assets, config)
    });
}
