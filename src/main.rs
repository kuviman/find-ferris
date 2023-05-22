use geng::prelude::*;

struct Game {
    geng: Geng,
}

impl Game {
    pub fn new(geng: &Geng) -> Self {
        Self {
            geng: geng.clone(),
        }
    }
}

impl geng::State for Game {
    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        ugli::clear(framebuffer, Some(Rgba::BLACK), None, None);
    }
}

fn main() {
    let geng = Geng::new("Find Ferris");
    geng.clone().run_loading(async move { Game::new(&geng) });
}
