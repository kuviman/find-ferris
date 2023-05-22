use geng::prelude::*;

struct Game {
    geng: Geng,
    camera: geng::Camera2d,
}

impl Game {
    pub fn new(geng: &Geng) -> Self {
        Self {
            geng: geng.clone(),
            camera: geng::Camera2d {
                center: vec2::ZERO,
                rotation: 0.0,
                fov: 10.0,
            },
        }
    }
}

impl geng::State for Game {
    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        ugli::clear(framebuffer, Some(Rgba::BLACK), None, None);
        const N: i32 = 10;
        for i in -N..=N {
            self.geng.draw2d().draw2d(
                framebuffer,
                &self.camera,
                &draw2d::Segment::new(
                    Segment(vec2(i as f32, -N as f32), vec2(i as f32, N as f32)),
                    0.1,
                    Rgba::GRAY,
                ),
            );
            self.geng.draw2d().draw2d(
                framebuffer,
                &self.camera,
                &draw2d::Segment::new(
                    Segment(vec2(-N as f32, i as f32), vec2(N as f32, i as f32)),
                    0.1,
                    Rgba::GRAY,
                ),
            );
        }
    }
}

fn main() {
    let geng = Geng::new("Find Ferris");
    geng.clone().run_loading(async move { Game::new(&geng) });
}
