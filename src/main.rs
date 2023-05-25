use geng::prelude::*;

enum Drag {
    None,
    Detecting { from: vec2<f32>, timer: Timer },
    Dragging { prev_mouse_pos: vec2<f32> },
}

#[derive(Deserialize)]
struct Config {
    pub crabs: usize,
    pub min_drag_distance: f32,
    pub default_fov: f32,
    pub drag_start_timer: f64, // TODO: Duration
    pub crab_speed: f32,
    pub road_node_ui_radius: f32,
    pub zoom_speed: f32,
    pub min_fov: f32,
    pub max_fov: f32,
    pub animation_speed: f32,
    pub jump_height: f32,
    pub jump_rotation_amplitude: f32,
    pub collision_check_distance: f32,
    pub collision_check_radius: f32,
    pub collision_slow_down: f32,
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

fn fix_roads(roads: &mut Roads) {
    for (index, node) in roads.nodes.iter_mut().enumerate() {
        node.connected.retain(|&other| other != index);
    }
}

impl Roads {
    pub fn world_pos(&self, position: &Position) -> vec2<f32> {
        let from = self.nodes[position.from].pos;
        let to = match position.to {
            Some(to) => self.nodes[to].pos,
            None => return from,
        };
        from + (to - from).normalize() * position.distance
    }
}

#[derive(geng::asset::Load)]
struct Assets {
    pub ferris_pirate: ugli::Texture,
    pub ground: ugli::Texture,
    pub obstacles: ugli::Texture,
    #[load(postprocess = "fix_roads")]
    pub roads: Roads,
}

struct Position {
    from: NodeId,
    to: Option<NodeId>,
    distance: f32,
}

struct Crab {
    position: Position,
    animation_time: f32,
}

struct Editor {
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
    editor: Editor,
}

impl Game {
    pub fn new(geng: &Geng, assets: Assets, config: Config) -> Self {
        let crabs_count = config.crabs;
        let mut result = Self {
            geng: geng.clone(),
            framebuffer_size: vec2::splat(1.0),
            camera: geng::Camera2d {
                center: vec2::ZERO,
                rotation: 0.0,
                fov: config.default_fov,
            },
            drag: Drag::None,
            crabs: vec![],
            config,
            assets,
            editor: Editor {
                drag_from: None,
                shown: false,
            },
        };
        for _ in 0..crabs_count {
            result.spawn_crab();
        }
        result
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

    fn clamp_camera(&mut self) {
        let map_size = self.assets.ground.size().map(|x| x as f32);
        self.camera.fov = self
            .camera
            .fov
            .min(map_size.y as f32)
            .min(map_size.x / self.framebuffer_size.aspect());

        let possible_positions = Aabb2::ZERO
            .extend_symmetric(map_size / 2.0)
            .extend_symmetric(-vec2(
                self.camera.fov / 2.0 * self.framebuffer_size.aspect(),
                self.camera.fov / 2.0,
            ));
        self.camera.center = self.camera.center.clamp_aabb(possible_positions);
    }

    fn spawn_crab(&mut self) {
        let from = thread_rng().gen_range(0..self.assets.roads.nodes.len());
        let to = self.assets.roads.nodes[from]
            .connected
            .choose(&mut thread_rng())
            .copied();
        let distance = match to {
            Some(to) => {
                assert!(to != from);
                thread_rng().gen_range(
                    0.0..(self.assets.roads.nodes[from].pos - self.assets.roads.nodes[to].pos)
                        .len(),
                )
            }
            None => 0.0,
        };
        self.crabs.push(Crab {
            position: Position { from, to, distance },
            animation_time: thread_rng().gen(),
        });
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

        for crab_index in 0..self.crabs.len() {
            let slow_down = {
                let mut slow_down = 1.0;
                let crab = &self.crabs[crab_index];
                let front_pos = self.assets.roads.world_pos(&Position {
                    distance: crab.position.distance + self.config.collision_check_distance,
                    ..crab.position
                });
                for other_index in 0..self.crabs.len() {
                    if other_index == crab_index {
                        continue;
                    }
                    let pos = self
                        .assets
                        .roads
                        .world_pos(&self.crabs[other_index].position);
                    if (front_pos - pos).len() < self.config.collision_check_radius {
                        slow_down *= self.config.collision_slow_down;
                    }
                }
                slow_down
            };
            let crab = &mut self.crabs[crab_index];
            let position = &mut crab.position;
            if let Some(to) = position.to {
                position.distance += self.config.crab_speed / slow_down * delta_time;
                if position.distance
                    > (self.assets.roads.nodes[position.from].pos - self.assets.roads.nodes[to].pos)
                        .len()
                {
                    *position = Position {
                        from: to,
                        to: self.assets.roads.nodes[to]
                            .connected
                            .choose(&mut thread_rng())
                            .copied(),
                        distance: 0.0,
                    };
                }
            }

            crab.animation_time += self.config.animation_speed * delta_time;
        }
    }
    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        self.framebuffer_size = framebuffer.size().map(|x| x as f32);
        ugli::clear(framebuffer, Some(Rgba::BLACK), None, None);

        self.clamp_camera();

        let mut draw_sprite = |texture: &ugli::Texture, transform: mat3<f32>| {
            self.geng.draw2d().draw2d(
                framebuffer,
                &self.camera,
                &draw2d::TexturedQuad::new(
                    Aabb2::point(vec2::ZERO)
                        .extend_symmetric(texture.size().map(|x| x as f32) / 2.0),
                    texture,
                )
                .transform(transform),
            );
        };

        draw_sprite(&self.assets.ground, mat3::identity());
        let mut crab_indices: Vec<usize> = (0..self.crabs.len()).collect();
        crab_indices
            .sort_by_key(|index| -r32(self.assets.roads.world_pos(&self.crabs[*index].position).y));
        for index in crab_indices {
            let crab = &self.crabs[index];
            let pos = self.assets.roads.world_pos(&crab.position);
            draw_sprite(
                &self.assets.ferris_pirate,
                mat3::translate(
                    pos + vec2(
                        0.0,
                        crab.animation_time.cos().abs() * self.config.jump_height,
                    ),
                ) * mat3::rotate(crab.animation_time.sin() * self.config.jump_rotation_amplitude),
            );
        }
        draw_sprite(&self.assets.obstacles, mat3::identity());

        // for crab in &self.crabs {
        //     let pos = self.assets.roads.world_pos(&crab.position);
        //     self.geng.draw2d().draw2d(
        //         framebuffer,
        //         &self.camera,
        //         &draw2d::Ellipse::circle(pos, self.config.collision_check_radius, Rgba::RED),
        //     );
        // }

        // Road editor
        if self.editor.shown {
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
                    geng::Key::Tab => self.editor.shown = !self.editor.shown,
                    geng::Key::N => self.assets.roads.nodes.push(RoadNode {
                        pos: cursor_world,
                        connected: default(),
                    }),
                    geng::Key::E => {
                        // TODO make engine not send repeated key or smth
                        if self.editor.drag_from.is_none() {
                            self.editor.drag_from = dbg!(self.hovered_road_node());
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
                        self.spawn_crab();
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
                    if let Some(from) = self.editor.drag_from.take() {
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
