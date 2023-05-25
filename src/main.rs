use geng::prelude::*;

enum Drag {
    None,
    Detecting { from: vec2<f32>, timer: Timer },
    Dragging { prev_mouse_pos: vec2<f32> },
}

#[derive(Deserialize)]
struct Config {
    pub min_drag_distance: f32,
    pub default_fov: f32,
    pub drag_start_timer: f64, // TODO: Duration
    pub crab_speed: f32,
    pub road_node_ui_radius: f32,
    pub zoom_speed: f32,
    pub min_fov: f32,
    pub max_fov: f32,
}

type NodeId = usize;

#[derive(Serialize, Deserialize)]
struct RoadNode {
    pos: vec2<f32>,
    connected: Vec<NodeId>,
}

#[derive(geng::asset::Load, Serialize, Deserialize)]
#[load(json)]
struct Roads {
    nodes: Vec<RoadNode>,
}

#[derive(geng::asset::Load)]
struct Assets {
    pub ferris_pirate: ugli::Texture,
    pub ground: ugli::Texture,
    pub obstacles: ugli::Texture,
    pub roads: Roads,
}

struct Crab {
    from: NodeId,
    to: Option<NodeId>,
    distance: f32,
}

struct RoadEditor {
    drag_from: Option<usize>,
    shown: bool,
}

struct Game {
    geng: Geng,
    framebuffer_size: vec2<f32>,
    camera: geng::Camera2d,
    drag: Drag,
    config: Config,
    assets: Assets,
    crabs: Vec<Crab>,
    road_editor: RoadEditor,
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
                fov: config.default_fov,
            },
            drag: Drag::None,
            config,
            crabs: vec![],
            road_editor: RoadEditor {
                drag_from: None,
                shown: true,
            },
        }
    }

    fn hovered_road_node(&self) -> Option<NodeId> {
        let cursor = self.camera.screen_to_world(
            self.framebuffer_size,
            self.geng.window().cursor_position().map(|x| x as f32),
        );
        self.assets
            .roads
            .nodes
            .iter()
            .position(|node| (node.pos - cursor).len() < self.config.road_node_ui_radius)
    }
}

impl geng::State for Game {
    fn update(&mut self, delta_time: f64) {
        let delta_time = delta_time as f32;

        if let Drag::Detecting { from, timer } = &self.drag {
            if timer.elapsed().as_secs_f64() > self.config.drag_start_timer {
                self.drag = Drag::Dragging {
                    prev_mouse_pos: *from,
                };
            }
        }

        for crab in &mut self.crabs {
            if let Some(to) = crab.to {
                crab.distance += self.config.crab_speed * delta_time;
                if crab.distance
                    > (self.assets.roads.nodes[crab.from].pos - self.assets.roads.nodes[to].pos)
                        .len()
                {
                    *crab = Crab {
                        from: to,
                        to: self.assets.roads.nodes[to]
                            .connected
                            .choose(&mut thread_rng())
                            .copied(),
                        distance: 0.0,
                    };
                }
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
        for crab in &self.crabs {
            let from = self.assets.roads.nodes[crab.from].pos;
            let to = match crab.to {
                Some(to) => self.assets.roads.nodes[to].pos,
                None => from,
            };
            let pos = from + (to - from).normalize() * crab.distance;
            draw_sprite(&self.assets.ferris_pirate, pos);
        }
        draw_sprite(&self.assets.obstacles, vec2::ZERO);

        // Road editor
        if self.road_editor.shown {
            for node in &self.assets.roads.nodes {
                self.geng.draw2d().draw2d(
                    framebuffer,
                    &self.camera,
                    &draw2d::Ellipse::circle(
                        node.pos,
                        self.config.road_node_ui_radius,
                        Rgba::GREEN,
                    ),
                );
            }
            for from in &self.assets.roads.nodes {
                for &to in &from.connected {
                    let to = &self.assets.roads.nodes[to];
                    self.geng.draw2d().draw2d(
                        framebuffer,
                        &self.camera,
                        &draw2d::Segment::new_gradient(
                            draw2d::ColoredVertex {
                                a_pos: from.pos,
                                a_color: Rgba::BLUE,
                            },
                            draw2d::ColoredVertex {
                                a_pos: to.pos,
                                a_color: Rgba::RED,
                            },
                            self.config.road_node_ui_radius * 0.5,
                        ),
                    );
                }
            }
            if let Some(index) = self.hovered_road_node() {
                self.geng.draw2d().draw2d(
                    framebuffer,
                    &self.camera,
                    &draw2d::Ellipse::circle_with_cut(
                        self.assets.roads.nodes[index].pos,
                        self.config.road_node_ui_radius * 1.1,
                        self.config.road_node_ui_radius * 1.2,
                        Rgba::new(1.0, 1.0, 1.0, 0.5),
                    ),
                );
            }
        }
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
            geng::Event::Wheel { delta } => {
                let cursor = self.geng.window().cursor_position().map(|x| x as f32);
                let prev_world_cursor = world_pos(cursor);
                self.camera.fov = (self.camera.fov * self.config.zoom_speed.powf(-delta as f32))
                    .clamp(self.config.min_fov, self.config.max_fov);
                let new_world_cursor = self.camera.screen_to_world(self.framebuffer_size, cursor);
                self.camera.center += prev_world_cursor - new_world_cursor;
            }
            geng::Event::KeyDown { key } => {
                let cursor_world =
                    world_pos(self.geng.window().cursor_position().map(|x| x as f32));
                match key {
                    geng::Key::Tab => self.road_editor.shown = !self.road_editor.shown,
                    geng::Key::N => self.assets.roads.nodes.push(RoadNode {
                        pos: cursor_world,
                        connected: default(),
                    }),
                    geng::Key::E => {
                        // TODO make engine not send repeated key or smth
                        if self.road_editor.drag_from.is_none() {
                            self.road_editor.drag_from = dbg!(self.hovered_road_node());
                        }
                    }
                    geng::Key::Delete => {
                        if let Some(index) = self.hovered_road_node() {
                            self.assets.roads.nodes.remove(index);
                            for node in &mut self.assets.roads.nodes {
                                node.connected.retain(|v| *v != index);
                                for to in &mut node.connected {
                                    if *to > index {
                                        *to -= 1;
                                    }
                                }
                            }
                        }
                    }
                    geng::Key::Space => {
                        let from = thread_rng().gen_range(0..self.assets.roads.nodes.len());
                        let to = self.assets.roads.nodes[from]
                            .connected
                            .choose(&mut thread_rng())
                            .copied();
                        let distance = 0.0;
                        self.crabs.push(Crab { from, to, distance });
                    }
                    geng::Key::R => {
                        self.crabs.clear();
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    geng::Key::S if self.geng.window().is_key_pressed(geng::Key::LCtrl) => {
                        let mut f = std::io::BufWriter::new(
                            std::fs::File::create(run_dir().join("assets").join("roads.json"))
                                .unwrap(),
                        );
                        serde_json::to_writer(&mut f, &self.assets.roads).unwrap();
                    }
                    _ => {}
                }
            }
            geng::Event::KeyUp { key } => match key {
                geng::Key::E => {
                    if let Some(from) = self.road_editor.drag_from.take() {
                        if let Some(to) = self.hovered_road_node() {
                            self.assets.roads.nodes[from].connected.push(to);
                        }
                    }
                }
                _ => {}
            },
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
