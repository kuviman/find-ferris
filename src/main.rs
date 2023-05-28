use geng::prelude::*;

#[derive(Deref)]
pub struct Toml<T>(#[deref] pub T);

impl<T: DeserializeOwned + 'static> geng::asset::Load for Toml<T> {
    fn load(_manager: &geng::asset::Manager, path: &std::path::Path) -> geng::asset::Future<Self> {
        let path = path.to_owned();
        async move { Ok(Self(file::load_detect(path).await?)) }.boxed_local()
    }
    const DEFAULT_EXT: Option<&'static str> = Some("toml");
}

enum Drag {
    None,
    Detecting { from: vec2<f32>, timer: Timer },
    Dragging { prev_mouse_pos: vec2<f32> },
}

#[derive(Deserialize)]
struct Config {
    pub click_radius: f32,
    pub crabs: usize,
    pub free_items: usize,
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
    pub crab_hold_item_probability: f64,
    pub crab_hold_double_item_probability: f64,
    pub crab_left_hand_pos: vec2<f32>,
    pub crab_right_hand_pos: vec2<f32>,
    pub types_to_find: usize,
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

#[derive(geng::asset::Load, Serialize, Deserialize, Deref, DerefMut)]
#[serde(transparent)]
#[load(json)]
struct ItemPositions {
    #[deref]
    positions: Vec<vec2<f32>>,
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

#[derive(Deserialize)]
pub struct CrabConfig {
    pub spawn_weight: f64,
}

#[derive(geng::asset::Load)]
pub struct CrabAssets {
    pub config: Toml<CrabConfig>,
    pub texture: ugli::Texture,
}

#[derive(Deserialize)]
struct WheelConfig {
    pub pos: vec2<f32>,
    pub origin: vec2<f32>,
    pub base_shift: vec2<f32>,
    pub radius: f32,
    pub rotate_speed: f32, // TODO Angle
    pub cabins: usize,
    pub swing_origin: vec2<f32>,
    pub swing_freq: f32,
    pub swing_amplitude: f32,
    pub crab_pos: vec2<f32>,
    pub crab_scale: f32,
}

#[derive(geng::asset::Load)]
struct WheelAssets {
    pub config: Toml<WheelConfig>,
    pub base: ugli::Texture,
    pub wheel: ugli::Texture,
    pub cabin: ugli::Texture,
}

#[derive(geng::asset::Load)]
struct Assets {
    #[load(listed_in = "_list.ron")]
    pub crabs: Vec<CrabAssets>,
    pub ground: ugli::Texture,
    pub obstacles: ugli::Texture,
    #[load(postprocess = "fix_roads")]
    pub roads: Roads,
    pub wheel: WheelAssets,
    #[load(listed_in = "_list.ron")]
    pub items: Vec<ugli::Texture>,
    pub item_positions: ItemPositions,
    #[load(path = "font/Pangolin-Regular.ttf")]
    pub font: geng::Font,
    pub to_find_background: ugli::Texture,
}

struct Position {
    from: NodeId,
    to: Option<NodeId>,
    distance: f32,
}

struct Crab {
    type_index: usize,
    position: Position,
    animation_time: f32,
    left_hand: Option<usize>,
    right_hand: Option<usize>,
}

struct Editor {
    drag_from: Option<usize>,
    shown: bool,
}

type ItemType = usize;

struct Item {
    pub type_index: ItemType,
    pub pos_index: usize,
    pub rot: f32,
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
    current_time: f32,
    items: Vec<Item>,
    to_find: Vec<ItemType>,
}

impl Game {
    pub fn new(geng: &Geng, assets: Assets, config: Config) -> Self {
        let crabs_count = config.crabs;
        let free_items = config.free_items;
        let mut result = Self {
            current_time: 0.0,
            geng: geng.clone(),
            framebuffer_size: vec2::splat(1.0),
            camera: geng::Camera2d {
                center: vec2::ZERO,
                rotation: 0.0,
                fov: config.default_fov,
            },
            drag: Drag::None,
            crabs: vec![],
            to_find: rand::seq::index::sample(
                &mut thread_rng(),
                assets.items.len(),
                config.types_to_find,
            )
            .into_iter()
            .collect(),
            config,
            assets,
            editor: Editor {
                drag_from: None,
                shown: false,
            },
            items: vec![],
        };
        for _ in 0..crabs_count {
            result.spawn_crab();
        }
        for _ in 0..free_items {
            result.spawn_item();
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

    fn spawn_item(&mut self) {
        if let Some(index) = (0..self.assets.item_positions.len())
            .filter(|index| !self.items.iter().any(|item| item.pos_index == *index))
            .choose(&mut thread_rng())
        {
            self.items.push(Item {
                pos_index: index,
                type_index: thread_rng().gen_range(0..self.assets.items.len()),
                rot: thread_rng().gen_range(0.0..2.0 * f32::PI),
            });
        }
    }

    fn spawn_crab(&mut self) {
        let indices: Vec<usize> = (0..self.assets.roads.nodes.len())
            .filter(|index| {
                !self.assets.roads.nodes[*index].connected.is_empty()
                    || self.crabs.iter().all(|crab| crab.position.from != *index)
            })
            .collect();
        let from = *indices.choose(&mut thread_rng()).unwrap();
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
        let random_item = || thread_rng().gen_range(0..self.assets.items.len());
        let (left_hand, right_hand) =
            if thread_rng().gen_bool(self.config.crab_hold_item_probability) {
                if thread_rng().gen_bool(self.config.crab_hold_double_item_probability) {
                    (Some(random_item()), Some(random_item()))
                } else if thread_rng().gen() {
                    (Some(random_item()), None)
                } else {
                    (None, Some(random_item()))
                }
            } else {
                (None, None)
            };
        self.crabs.push(Crab {
            left_hand,
            right_hand,
            type_index: thread_rng().sample(
                rand::distributions::WeightedIndex::new(
                    self.assets
                        .crabs
                        .iter()
                        .map(|crab| crab.config.spawn_weight),
                )
                .unwrap(),
            ),
            position: Position { from, to, distance },
            animation_time: thread_rng().gen(),
        });
    }

    fn item_count(&self, item_type: ItemType) -> usize {
        let ground_items = self
            .items
            .iter()
            .filter(|item| item.type_index == item_type)
            .count();
        let crab_items = self
            .crabs
            .iter()
            .flat_map(|crab| [&crab.left_hand, &crab.right_hand])
            .filter(|hand| **hand == Some(item_type))
            .count();
        ground_items + crab_items
    }

    fn click(&mut self, pos: vec2<f32>) {
        let cursor_world = self.camera.screen_to_world(self.framebuffer_size, pos);
        let trigger_radius = self.config.click_radius;
        let can_take = |item: Option<ItemType>| -> bool {
            match item {
                Some(item) => self.to_find.contains(&item),
                None => false,
            }
        };
        // Ground
        if let Some(item) = self.items.iter().position(|item| {
            can_take(Some(item.type_index))
                && (self.assets.item_positions[item.pos_index] - cursor_world).len()
                    < trigger_radius
        }) {
            self.items.remove(item);
        }

        // Crab
        for i in (0..self.crabs.len()).rev() {
            let crab = &self.crabs[i];
            let check = |matrix: mat3<f32>| -> bool {
                let pos = (matrix * vec3(0.0, 0.0, 1.0)).into_2d();
                (pos - cursor_world).len() < trigger_radius
            };
            if check(self.crab_matrix_left_hand(crab)) && can_take(crab.left_hand) {
                self.crabs[i].left_hand = None;
            } else if check(self.crab_matrix_right_hand(crab)) && can_take(crab.right_hand) {
                self.crabs[i].right_hand = None;
            }
        }
    }

    fn crab_matrix(&self, crab: &Crab) -> mat3<f32> {
        let pos = self.assets.roads.world_pos(&crab.position);
        if crab.position.to.is_some() {
            mat3::translate(
                pos + vec2(
                    0.0,
                    crab.animation_time.cos().abs() * self.config.jump_height,
                ),
            ) * mat3::rotate(crab.animation_time.sin() * self.config.jump_rotation_amplitude)
        } else {
            mat3::translate(
                pos + vec2(
                    0.0,
                    crab.animation_time.cos().abs() * self.config.jump_height,
                ),
            )
        }
    }

    fn crab_matrix_left_hand(&self, crab: &Crab) -> mat3<f32> {
        self.crab_matrix(crab) * mat3::translate(self.config.crab_left_hand_pos)
    }

    fn crab_matrix_right_hand(&self, crab: &Crab) -> mat3<f32> {
        self.crab_matrix(crab) * mat3::translate(self.config.crab_right_hand_pos)
    }
}

impl geng::State for Game {
    fn update(&mut self, delta_time: f64) {
        let delta_time = delta_time as f32;

        self.current_time += delta_time;

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
            draw_sprite(
                &self.assets.crabs[crab.type_index].texture,
                self.crab_matrix(crab),
            );
            if let Some(item) = crab.left_hand {
                draw_sprite(&self.assets.items[item], self.crab_matrix_left_hand(crab));
            }
            if let Some(item) = crab.right_hand {
                draw_sprite(&self.assets.items[item], self.crab_matrix_right_hand(crab));
            }
        }
        draw_sprite(&self.assets.obstacles, mat3::identity());

        for item in &self.items {
            draw_sprite(
                &self.assets.items[item.type_index],
                mat3::translate(self.assets.item_positions[item.pos_index])
                    * mat3::rotate(item.rot),
            );
        }

        // Ferris wheel
        let wheel_rotation = self.current_time * self.assets.wheel.config.rotate_speed.to_radians();
        draw_sprite(
            &self.assets.wheel.base,
            mat3::translate(self.assets.wheel.config.pos + self.assets.wheel.config.base_shift),
        );
        draw_sprite(
            &self.assets.wheel.wheel,
            mat3::translate(self.assets.wheel.config.pos)
                * mat3::rotate(wheel_rotation)
                * mat3::translate(-self.assets.wheel.config.origin),
        );
        for i in 0..self.assets.wheel.config.cabins {
            let cabin_pos = self.assets.wheel.config.pos
                + vec2(self.assets.wheel.config.radius, 0.0).rotate(
                    2.0 * f32::PI * i as f32 / self.assets.wheel.config.cabins as f32
                        + wheel_rotation,
                );
            let cabin_transform = mat3::translate(cabin_pos)
                * mat3::rotate(
                    (2.0 * f32::PI * self.current_time * self.assets.wheel.config.swing_freq).sin()
                        * self.assets.wheel.config.swing_amplitude.to_radians(),
                )
                * mat3::translate(-self.assets.wheel.config.swing_origin);
            draw_sprite(
                &self.assets.crabs[i % self.assets.crabs.len()].texture,
                cabin_transform
                    * mat3::translate(self.assets.wheel.config.crab_pos)
                    * mat3::scale_uniform(self.assets.wheel.config.crab_scale),
            );
            draw_sprite(&self.assets.wheel.cabin, cabin_transform);
        }

        // Debug wheel
        if self.editor.shown {
            for i in 0..self.assets.wheel.config.cabins {
                self.geng.draw2d().draw2d(
                    framebuffer,
                    &self.camera,
                    &draw2d::Ellipse::circle(
                        self.assets.wheel.config.pos
                            + vec2(self.assets.wheel.config.radius, 0.0).rotate(
                                2.0 * f32::PI * i as f32 / self.assets.wheel.config.cabins as f32
                                    + wheel_rotation,
                            ),
                        self.config.collision_check_radius / 10.0,
                        Rgba::RED,
                    ),
                );
            }
            self.geng.draw2d().draw2d(
                framebuffer,
                &self.camera,
                &draw2d::Ellipse::circle(
                    self.assets.wheel.config.pos,
                    self.config.collision_check_radius,
                    Rgba::RED,
                ),
            );
        }

        let ui_camera = geng::Camera2d {
            center: vec2::ZERO,
            rotation: 0.0,
            fov: 11.0,
        };

        if !self.to_find.is_empty() {
            let total_width = self.to_find.len() as f32;
            self.geng.draw2d().draw2d(
                framebuffer,
                &ui_camera,
                &draw2d::TexturedQuad::new(
                    Aabb2::point(vec2(0.0, -4.0))
                        .extend_symmetric(vec2(total_width / 2.0 + 1.0, 0.0))
                        .extend_up(0.5)
                        .extend_down(1.3),
                    &self.assets.to_find_background,
                ),
            );
            for (i, &item) in self.to_find.iter().enumerate() {
                let number = self.item_count(item);
                let pos = vec2(-total_width / 2.0 + i as f32 + 0.5, -4.0);
                self.geng.draw2d().draw2d(
                    framebuffer,
                    &ui_camera,
                    &draw2d::Text::unit(&self.assets.font, number.to_string(), Rgba::BLACK)
                        .scale_uniform(0.2)
                        .translate(pos),
                );
                self.geng.draw2d().draw2d(
                    framebuffer,
                    &ui_camera,
                    &draw2d::TexturedQuad::unit(&self.assets.items[item])
                        .scale_uniform(0.5)
                        .translate(pos + vec2(0.0, -0.7)),
                );
            }
        }

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
            for position in self.assets.item_positions.iter() {
                self.geng.draw2d().draw2d(
                    framebuffer,
                    &self.camera,
                    &draw2d::Ellipse::circle(
                        *position,
                        self.config.road_node_ui_radius,
                        Rgba::BLUE,
                    ),
                );
            }
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
                    self.click(pos);
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
                    geng::Key::I => self.assets.item_positions.push(cursor_world),
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
                        if self.geng.window().is_key_pressed(geng::Key::LCtrl) {
                            self.spawn_crab();
                        } else {
                            self.spawn_item();
                        }
                    }
                    geng::Key::R => {
                        self.crabs.clear();
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    geng::Key::S if self.geng.window().is_key_pressed(geng::Key::LCtrl) => {
                        // save roads
                        let mut f = std::io::BufWriter::new(
                            std::fs::File::create(run_dir().join("assets").join("roads.json"))
                                .unwrap(),
                        );
                        serde_json::to_writer(&mut f, &self.assets.roads).unwrap();

                        // save item positions
                        let mut f = std::io::BufWriter::new(
                            std::fs::File::create(
                                run_dir().join("assets").join("item_positions.json"),
                            )
                            .unwrap(),
                        );
                        serde_json::to_writer(&mut f, &self.assets.item_positions).unwrap();
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
