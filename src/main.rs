extern crate cgmath;
extern crate fnv;
#[macro_use]
extern crate gfx;
extern crate gfx_device_gl;
extern crate gfx_window_glutin;
extern crate gilrs;
extern crate glutin;
extern crate image;

use gfx::Device;
use glutin::GlContext;

use cgmath::{InnerSpace, Vector2, vec2};
use fnv::FnvHashMap;

type ColourFormat = gfx::format::Srgba8;
type DepthFormat = gfx::format::DepthStencil;
type Resources = gfx_device_gl::Resources;

const QUAD_INDICES: [u16; 6] = [0, 1, 2, 2, 3, 0];
const QUAD_COORDS: [[f32; 2]; 4] = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];

const MAX_NUM_QUADS: usize = 1024;

struct RenderTarget<R: gfx::Resources> {
    rtv: gfx::handle::RenderTargetView<R, ColourFormat>,
    dsv: gfx::handle::DepthStencilView<R, DepthFormat>,
    srv: gfx::handle::ShaderResourceView<R, [f32; 4]>,
    dimensions: Vector2<u16>,
}

impl<R: gfx::Resources> RenderTarget<R> {
    pub fn new<F>(dimensions: Vector2<u16>, factory: &mut F) -> Self
    where
        F: gfx::Factory<R> + gfx::traits::FactoryExt<R>,
    {
        let (_, srv, rtv) = factory
            .create_render_target(dimensions.x, dimensions.y)
            .expect("Failed to create render target");
        let (_, _, dsv) = factory
            .create_depth_stencil(dimensions.x, dimensions.y)
            .expect("Failed to create depth stencil");
        Self {
            srv,
            rtv,
            dsv,
            dimensions,
        }
    }
}

gfx_constant_struct!(LightingProperties {
    output_size_in_pixels: [f32; 2] = "u_OutputSizeInPixels",
});

gfx_pipeline!(lighting_pipe {
    quad_corners: gfx::VertexBuffer<QuadCorners> = (),
    properties: gfx::ConstantBuffer<LightingProperties> = "Properties",
    in_colour: gfx::TextureSampler<[f32; 4]> = "t_Colour",
    in_visibility: gfx::TextureSampler<[f32; 4]> = "t_Visibility",
    out_colour: gfx::BlendTarget<ColourFormat> =
        ("Target0", gfx::state::ColorMask::all(), gfx::preset::blend::ALPHA),
});

struct LightingRenderer<R: gfx::Resources> {
    bundle: gfx::Bundle<R, lighting_pipe::Data<R>>,
    visibility_srv: gfx::handle::ShaderResourceView<R, [f32; 4]>,
}

impl<R: gfx::Resources> LightingRenderer<R> {
    pub fn new<F, C>(
        colour_srv: gfx::handle::ShaderResourceView<R, [f32; 4]>,
        visibility_srv: gfx::handle::ShaderResourceView<R, [f32; 4]>,
        rtv: gfx::handle::RenderTargetView<R, ColourFormat>,
        factory: &mut F,
        encoder: &mut gfx::Encoder<R, C>,
    ) -> Self
    where
        F: gfx::Factory<R> + gfx::traits::FactoryExt<R>,
        C: gfx::CommandBuffer<R>,
    {
        let sampler = factory.create_sampler(gfx::texture::SamplerInfo::new(
            gfx::texture::FilterMethod::Mipmap,
            gfx::texture::WrapMode::Tile,
        ));

        let pso = factory
            .create_pipeline_simple(
                include_bytes!("shaders/lighting/shader.150.vert"),
                include_bytes!("shaders/lighting/shader.150.frag"),
                lighting_pipe::new(),
            )
            .expect("Failed to create pipeline");

        let quad_corners_data = QUAD_COORDS
            .iter()
            .map(|v| QuadCorners {
                corner_zero_to_one: *v,
            })
            .collect::<Vec<_>>();

        let (quad_corners_buf, slice) = factory
            .create_vertex_buffer_with_slice(&quad_corners_data, &QUAD_INDICES[..]);

        let data = lighting_pipe::Data {
            quad_corners: quad_corners_buf,
            properties: factory.create_constant_buffer(1),
            in_colour: (colour_srv, sampler.clone()),
            in_visibility: (visibility_srv.clone(), sampler.clone()),
            out_colour: rtv,
        };
        let bundle = gfx::pso::bundle::Bundle::new(slice, pso, data);
        let (window_width, window_height, _, _) = bundle.data.out_colour.get_dimensions();
        let properties = LightingProperties {
            output_size_in_pixels: [window_width as f32, window_height as f32],
        };
        encoder.update_constant_buffer(&bundle.data.properties, &properties);
        Self {
            bundle,
            visibility_srv,
        }
    }

    fn encode<C>(&self, encoder: &mut gfx::Encoder<R, C>)
    where
        C: gfx::CommandBuffer<R>,
    {
        encoder.generate_mipmap(&self.visibility_srv);
        self.bundle.encode(encoder);
    }
}

gfx_constant_struct!(MapProperties {
    output_size_in_pixels: [f32; 2] = "u_OutputSizeInPixels",
});

gfx_pipeline!(map_pipe {
    quad_corners: gfx::VertexBuffer<QuadCorners> = (),
    properties: gfx::ConstantBuffer<MapProperties> = "Properties",
    image: gfx::TextureSampler<[f32; 4]> = "t_Image",
    out_visibility: gfx::BlendTarget<ColourFormat> =
        ("TargetVisibility", gfx::state::ColorMask::all(), gfx::preset::blend::ALPHA),
    out_colour: gfx::BlendTarget<ColourFormat> =
        ("TargetColour", gfx::state::ColorMask::all(), gfx::preset::blend::ALPHA),
    out_depth: gfx::DepthTarget<DepthFormat> = gfx::preset::depth::LESS_EQUAL_WRITE,
});

struct MapRenderer<R: gfx::Resources> {
    bundle: gfx::Bundle<R, map_pipe::Data<R>>,
}

impl<R: gfx::Resources> MapRenderer<R> {
    pub fn new<F, C>(
        colour_rtv: gfx::handle::RenderTargetView<R, ColourFormat>,
        visibility_rtv: gfx::handle::RenderTargetView<R, ColourFormat>,
        dsv: gfx::handle::DepthStencilView<R, DepthFormat>,
        factory: &mut F,
        encoder: &mut gfx::Encoder<R, C>,
    ) -> Self
    where
        F: gfx::Factory<R> + gfx::traits::FactoryExt<R>,
        C: gfx::CommandBuffer<R>,
    {
        let image = image::load_from_memory(include_bytes!("images/map.png"))
            .expect("Failed to decode image")
            .to_rgba();

        let (image_width, image_height) = image.dimensions();
        let tex_kind = gfx::texture::Kind::D2(
            image_width as u16,
            image_height as u16,
            gfx::texture::AaMode::Single,
        );
        let tex_mipmap = gfx::texture::Mipmap::Allocated;
        let (_, texture_srv) = factory
            .create_texture_immutable_u8::<ColourFormat>(tex_kind, tex_mipmap, &[&image])
            .expect("failed to create texture");
        let sampler = factory.create_sampler(gfx::texture::SamplerInfo::new(
            gfx::texture::FilterMethod::Mipmap,
            gfx::texture::WrapMode::Tile,
        ));

        let pso = factory
            .create_pipeline_simple(
                include_bytes!("shaders/map/shader.150.vert"),
                include_bytes!("shaders/map/shader.150.frag"),
                map_pipe::new(),
            )
            .expect("Failed to create pipeline");

        let quad_corners_data = QUAD_COORDS
            .iter()
            .map(|v| QuadCorners {
                corner_zero_to_one: *v,
            })
            .collect::<Vec<_>>();

        let (quad_corners_buf, slice) = factory
            .create_vertex_buffer_with_slice(&quad_corners_data, &QUAD_INDICES[..]);

        let data = map_pipe::Data {
            quad_corners: quad_corners_buf,
            properties: factory.create_constant_buffer(1),
            image: (texture_srv, sampler),
            out_visibility: visibility_rtv,
            out_colour: colour_rtv,
            out_depth: dsv,
        };
        let bundle = gfx::pso::bundle::Bundle::new(slice, pso, data);
        let (window_width, window_height, _, _) = bundle.data.out_colour.get_dimensions();
        let properties = MapProperties {
            output_size_in_pixels: [window_width as f32, window_height as f32],
        };
        encoder.update_constant_buffer(&bundle.data.properties, &properties);
        Self { bundle }
    }

    fn encode<C>(&self, encoder: &mut gfx::Encoder<R, C>)
    where
        C: gfx::CommandBuffer<R>,
    {
        self.bundle.encode(encoder);
    }
}

gfx_vertex_struct!(QuadCorners {
    corner_zero_to_one: [f32; 2] = "a_CornerZeroToOne",
});

gfx_vertex_struct!(QuadInstance {
    position_of_centre_in_pixels: [f32; 2] = "i_PositionOfCentreInPixels",
    dimensions_in_pixels: [f32; 2] = "i_DimensionsInPixels",
    facing_vector: [f32; 2] = "i_FacingVector",
    sprite_position_of_top_left_in_pixels: [f32; 2] = "i_SpritePositionOfTopLeftInPixels",
    sprite_dimensions_in_pixels: [f32; 2] = "i_SpriteDimensionsInPixels",
});

gfx_constant_struct!(QuadProperties {
    window_size_in_pixels: [f32; 2] = "u_WindowSizeInPixels",
    sprite_sheet_size_in_pixels: [f32; 2] = "u_SpriteSheetSizeInPixels",
});

gfx_pipeline!(quad_pipe {
    quad_corners: gfx::VertexBuffer<QuadCorners> = (),
    quad_instances: gfx::InstanceBuffer<QuadInstance> = (),
    properties: gfx::ConstantBuffer<QuadProperties> = "Properties",
    sprite_sheet: gfx::TextureSampler<[f32; 4]> = "t_SpriteSheet",
    out_visibility: gfx::BlendTarget<ColourFormat> =
        ("TargetVisibility", gfx::state::ColorMask::all(), gfx::preset::blend::ALPHA),
    out_colour: gfx::BlendTarget<ColourFormat> =
        ("TargetColour", gfx::state::ColorMask::all(), gfx::preset::blend::ALPHA),
    out_depth: gfx::DepthTarget<DepthFormat> = gfx::preset::depth::LESS_EQUAL_WRITE,
});

struct QuadRenderer<R: gfx::Resources> {
    bundle: gfx::Bundle<R, quad_pipe::Data<R>>,
    num_quads: usize,
    quad_instances_upload: gfx::handle::Buffer<R, QuadInstance>,
}

impl<R: gfx::Resources> QuadRenderer<R> {
    pub fn new<F, C>(
        colour_rtv: gfx::handle::RenderTargetView<R, ColourFormat>,
        visibility_rtv: gfx::handle::RenderTargetView<R, ColourFormat>,
        dsv: gfx::handle::DepthStencilView<R, DepthFormat>,
        factory: &mut F,
        encoder: &mut gfx::Encoder<R, C>,
    ) -> Self
    where
        F: gfx::Factory<R> + gfx::traits::FactoryExt<R>,
        C: gfx::CommandBuffer<R>,
    {
        let image = image::load_from_memory(include_bytes!("images/sprites.png"))
            .expect("Failed to decode image")
            .to_rgba();
        let (image_width, image_height) = image.dimensions();
        let tex_kind = gfx::texture::Kind::D2(
            image_width as u16,
            image_height as u16,
            gfx::texture::AaMode::Single,
        );
        let tex_mipmap = gfx::texture::Mipmap::Allocated;
        let (_, texture_srv) = factory
            .create_texture_immutable_u8::<ColourFormat>(tex_kind, tex_mipmap, &[&image])
            .expect("failed to create texture");
        let sampler = factory.create_sampler(gfx::texture::SamplerInfo::new(
            gfx::texture::FilterMethod::Mipmap,
            gfx::texture::WrapMode::Tile,
        ));

        let pso = factory
            .create_pipeline_simple(
                include_bytes!("shaders/quad/shader.150.vert"),
                include_bytes!("shaders/quad/shader.150.frag"),
                quad_pipe::new(),
            )
            .expect("Failed to create pipeline");

        let quad_corners_data = QUAD_COORDS
            .iter()
            .map(|v| QuadCorners {
                corner_zero_to_one: *v,
            })
            .collect::<Vec<_>>();

        let (quad_corners_buf, slice) = factory
            .create_vertex_buffer_with_slice(&quad_corners_data, &QUAD_INDICES[..]);

        let data = quad_pipe::Data {
            quad_corners: quad_corners_buf,
            quad_instances: create_instance_buffer(MAX_NUM_QUADS, factory)
                .expect("Failed to create instance buffer"),
            properties: factory.create_constant_buffer(1),
            sprite_sheet: (texture_srv, sampler),
            out_colour: colour_rtv,
            out_visibility: visibility_rtv,
            out_depth: dsv,
        };
        let bundle = gfx::pso::bundle::Bundle::new(slice, pso, data);
        let (window_width, window_height, _, _) = bundle.data.out_colour.get_dimensions();
        let properties = QuadProperties {
            window_size_in_pixels: [window_width as f32, window_height as f32],
            sprite_sheet_size_in_pixels: [image_width as f32, image_height as f32],
        };

        let quad_instances_upload: gfx::handle::Buffer<R, QuadInstance> = factory
            .create_upload_buffer(MAX_NUM_QUADS)
            .expect("Failed to create instance upload buffer");
        encoder.update_constant_buffer(&bundle.data.properties, &properties);

        Self {
            bundle,
            num_quads: 0,
            quad_instances_upload,
        }
    }

    fn update<'a, F, I>(&mut self, to_render: I, factory: &mut F)
    where
        F: gfx::Factory<R> + gfx::traits::FactoryExt<R>,
        I: IntoIterator<Item = ToRender<'a>>,
    {
        let mut quad_instance_writer = factory
            .write_mapping(&self.quad_instances_upload)
            .expect("Failed to map upload buffer");
        self.num_quads = to_render
            .into_iter()
            .zip(quad_instance_writer.iter_mut())
            .fold(0, |count, (to_render, writer)| {
                writer.position_of_centre_in_pixels =
                    to_render.physics.centre_position.into();
                writer.dimensions_in_pixels =
                    to_render.physics.bounding_dimensions.into();
                writer.facing_vector = to_render.physics.facing.into();
                writer.sprite_position_of_top_left_in_pixels =
                    to_render.graphics.sprite_position_of_top_left_in_pixels;
                writer.sprite_dimensions_in_pixels =
                    to_render.graphics.sprite_dimensions_in_pixels;
                count + 1
            });
        self.bundle.slice.instances = Some((self.num_quads as u32, 0));
    }

    fn encode<C>(&self, encoder: &mut gfx::Encoder<R, C>)
    where
        C: gfx::CommandBuffer<R>,
    {
        encoder
            .copy_buffer(
                &self.quad_instances_upload,
                &self.bundle.data.quad_instances,
                0,
                0,
                self.num_quads,
            )
            .expect("Failed to copy instances");
        self.bundle.encode(encoder);
    }
}

fn create_instance_buffer<R, F, T>(
    size: usize,
    factory: &mut F,
) -> Result<gfx::handle::Buffer<R, T>, gfx::buffer::CreationError>
where
    R: gfx::Resources,
    F: gfx::Factory<R> + gfx::traits::FactoryExt<R>,
{
    factory.create_buffer(
        size,
        gfx::buffer::Role::Vertex,
        gfx::memory::Usage::Data,
        gfx::memory::Bind::TRANSFER_DST,
    )
}

enum ExternalEvent {
    Quit,
}

fn update_input_model(
    input_model: &mut InputModel,
    events_loop: &mut glutin::EventsLoop,
    gilrs: &mut gilrs::Gilrs,
) -> Option<ExternalEvent> {
    let mut external_event = None;
    input_model.progress_buttons();
    while let Some(event) = gilrs.next_event() {
        match event.event {
            gilrs::EventType::AxisChanged(axis, value, _) => match axis {
                gilrs::ev::Axis::LeftStickX => input_model.set_aim_x(value),
                gilrs::ev::Axis::LeftStickY => input_model.set_aim_y(-value),
                gilrs::ev::Axis::Unknown => input_model.set_thrust((value + 1.) / 2.),
                _ => (),
            },
            gilrs::EventType::ButtonPressed(button, _) => match button {
                gilrs::ev::Button::DPadUp => input_model.set_aim_y(1.),
                gilrs::ev::Button::DPadDown => input_model.set_aim_y(-1.),
                gilrs::ev::Button::DPadLeft => input_model.set_aim_x(-1.),
                gilrs::ev::Button::DPadRight => input_model.set_aim_x(1.),
                gilrs::ev::Button::RightTrigger => input_model.press_shoot(),
                gilrs::ev::Button::South => input_model.release_shoot(),
                _ => (),
            },
            gilrs::EventType::ButtonChanged(button, value, _) => match button {
                gilrs::ev::Button::DPadUp => input_model.set_aim_y(value),
                gilrs::ev::Button::DPadDown => input_model.set_aim_y(-value),
                gilrs::ev::Button::DPadLeft => input_model.set_aim_x(-value),
                gilrs::ev::Button::DPadRight => input_model.set_aim_x(value),
                _ => (),
            },
            gilrs::EventType::ButtonReleased(button, _) => match button {
                gilrs::ev::Button::DPadUp | gilrs::ev::Button::DPadDown => {
                    input_model.set_aim_y(0.)
                }
                gilrs::ev::Button::DPadLeft | gilrs::ev::Button::DPadRight => {
                    input_model.set_aim_x(0.)
                }
                gilrs::ev::Button::RightTrigger => input_model.release_shoot(),
                gilrs::ev::Button::South => input_model.release_shoot(),
                _ => (),
            },
            _ => (),
        }
    }

    events_loop.poll_events(|event| match event {
        glutin::Event::WindowEvent { event, .. } => match event {
            glutin::WindowEvent::CloseRequested => {
                external_event = Some(ExternalEvent::Quit);
            }
            glutin::WindowEvent::KeyboardInput { input, .. } => {
                if let Some(virtual_keycode) = input.virtual_keycode {
                    match input.state {
                        glutin::ElementState::Pressed => match virtual_keycode {
                            glutin::VirtualKeyCode::W => input_model.set_aim_y(-1.),
                            glutin::VirtualKeyCode::S => input_model.set_aim_y(1.),
                            glutin::VirtualKeyCode::A => input_model.set_aim_x(-1.),
                            glutin::VirtualKeyCode::D => input_model.set_aim_x(1.),
                            glutin::VirtualKeyCode::Comma => input_model.set_aim_y(-1.),
                            glutin::VirtualKeyCode::O => input_model.set_aim_y(1.),
                            glutin::VirtualKeyCode::E => input_model.set_aim_x(1.),
                            glutin::VirtualKeyCode::Return => input_model.press_shoot(),
                            glutin::VirtualKeyCode::Space => input_model.set_thrust(1.),
                            _ => (),
                        },
                        glutin::ElementState::Released => match virtual_keycode {
                            glutin::VirtualKeyCode::W => input_model.set_aim_y(0.),
                            glutin::VirtualKeyCode::S => input_model.set_aim_y(0.),
                            glutin::VirtualKeyCode::A => input_model.set_aim_x(0.),
                            glutin::VirtualKeyCode::D => input_model.set_aim_x(0.),
                            glutin::VirtualKeyCode::Comma => input_model.set_aim_y(0.),
                            glutin::VirtualKeyCode::O => input_model.set_aim_y(0.),
                            glutin::VirtualKeyCode::E => input_model.set_aim_x(0.),
                            glutin::VirtualKeyCode::Return => input_model.release_shoot(),
                            glutin::VirtualKeyCode::Space => input_model.set_thrust(0.),
                            _ => (),
                        },
                    }
                }
            }
            _ => (),
        },
        _ => (),
    });

    external_event
}

fn main() {
    let (width, height) = (1024, 1024);
    let builder = glutin::WindowBuilder::new()
        .with_dimensions(width, height)
        .with_min_dimensions(width, height)
        .with_max_dimensions(width, height);
    let mut events_loop = glutin::EventsLoop::new();
    let context = glutin::ContextBuilder::new().with_vsync(true);
    let (window, mut device, mut factory, rtv, dsv) = gfx_window_glutin::init::<
        ColourFormat,
        DepthFormat,
    >(builder, context, &events_loop);

    let mut encoder: gfx::Encoder<Resources, gfx_device_gl::CommandBuffer> =
        factory.create_command_buffer().into();

    let colour_target =
        RenderTarget::new(vec2(width as u16, height as u16), &mut factory);
    let visibility_target =
        RenderTarget::new(vec2(width as u16, height as u16), &mut factory);

    let mut quad_renderer = QuadRenderer::new(
        colour_target.rtv.clone(),
        visibility_target.rtv.clone(),
        colour_target.dsv.clone(),
        &mut factory,
        &mut encoder,
    );

    let mut map_renderer = MapRenderer::new(
        colour_target.rtv.clone(),
        visibility_target.rtv.clone(),
        colour_target.dsv.clone(),
        &mut factory,
        &mut encoder,
    );

    let mut lighting_renderer = LightingRenderer::new(
        colour_target.srv.clone(),
        visibility_target.srv.clone(),
        rtv.clone(),
        &mut factory,
        &mut encoder,
    );

    let mut gilrs = gilrs::Gilrs::new().unwrap();

    let mut game_state = GameState::new();
    let mut input_model = InputModel::default();
    loop {
        encoder.clear(&rtv, [0.0, 0.0, 0.0, 1.0]);
        encoder.clear_depth(&dsv, 1.0);
        match update_input_model(&mut input_model, &mut events_loop, &mut gilrs) {
            Some(ExternalEvent::Quit) => break,
            None => (),
        }
        game_state.update(&input_model);
        quad_renderer.update(game_state.to_render(), &mut factory);
        map_renderer.encode(&mut encoder);
        quad_renderer.encode(&mut encoder);
        lighting_renderer.encode(&mut encoder);

        encoder.flush(&mut device);
        window.swap_buffers().unwrap();
        device.cleanup();
    }
}

#[derive(Debug, Default)]
struct ButtonState {
    current: bool,
    previous: bool,
}

impl ButtonState {
    fn progress(&mut self) {
        self.previous = self.current;
    }
    fn press(&mut self) {
        self.current = true;
    }
    fn release(&mut self) {
        self.current = false;
    }
    fn is_press_transition(&self) -> bool {
        self.current && !self.previous
    }
}

#[derive(Debug)]
pub struct InputModel {
    aim_vec: Vector2<f32>,
    shoot: ButtonState,
    thrust: f32,
}

impl Default for InputModel {
    fn default() -> Self {
        Self {
            aim_vec: vec2(0., 0.),
            shoot: ButtonState::default(),
            thrust: 0.,
        }
    }
}

const ANALOG_THRESHOLD: f32 = 0.1;

fn analog_threshold_value(v: f32) -> f32 {
    if v.abs() > ANALOG_THRESHOLD {
        v
    } else {
        0.
    }
}

impl InputModel {
    pub fn progress_buttons(&mut self) {
        self.shoot.progress();
    }
    pub fn press_shoot(&mut self) {
        self.shoot.press();
    }
    pub fn release_shoot(&mut self) {
        self.shoot.release();
    }
    pub fn set_aim_x(&mut self, value: f32) {
        self.aim_vec.x = analog_threshold_value(value);
    }
    pub fn set_aim_y(&mut self, value: f32) {
        self.aim_vec.y = analog_threshold_value(value);
    }
    pub fn set_thrust(&mut self, value: f32) {
        self.thrust = analog_threshold_value(value).max(0.);
    }
    fn aim_vector(&self) -> Option<Vector2<f32>> {
        let magnitude2 = self.aim_vec.magnitude2();
        if magnitude2 >= 1. {
            Some(self.aim_vec.normalize())
        } else {
            const AIM_THRESHOLD2: f32 = 0.2;
            if magnitude2 > AIM_THRESHOLD2 {
                Some(self.aim_vec.normalize())
            } else {
                None
            }
        }
    }
}

type EntityId = u16;

#[derive(Default)]
struct EntityIdAllocator {
    next: EntityId,
}

impl EntityIdAllocator {
    fn allocate(&mut self) -> EntityId {
        let id = self.next;
        self.next += 1;
        id
    }
}

struct Physics {
    centre_position: Vector2<f32>,
    bounding_dimensions: Vector2<f32>,
    velocity: Vector2<f32>,
    facing: Vector2<f32>,
}

#[derive(Clone)]
struct Graphics {
    sprite_position_of_top_left_in_pixels: [f32; 2],
    sprite_dimensions_in_pixels: [f32; 2],
}

pub struct GameState {
    player_id: EntityId,
    entity_id_allocator: EntityIdAllocator,
    physics: FnvHashMap<EntityId, Physics>,
    graphics: FnvHashMap<EntityId, Graphics>,
}

pub struct ToRender<'a> {
    graphics: &'a Graphics,
    physics: &'a Physics,
}

impl GameState {
    pub fn new() -> Self {
        let mut entity_id_allocator = EntityIdAllocator::default();
        let player_id = entity_id_allocator.allocate();
        let mut game_state = Self {
            player_id,
            entity_id_allocator,
            physics: Default::default(),
            graphics: Default::default(),
        };
        game_state.physics.insert(
            player_id,
            Physics {
                centre_position: vec2(200., 100.),
                bounding_dimensions: vec2(32., 64.),
                velocity: vec2(0., 0.),
                facing: vec2(1., -1.).normalize(),
            },
        );
        game_state.graphics.insert(
            player_id,
            Graphics {
                sprite_position_of_top_left_in_pixels: [0., 0.],
                sprite_dimensions_in_pixels: [14., 26.],
            },
        );

        let asteroid_graphics = Graphics {
            sprite_position_of_top_left_in_pixels: [32., 0.],
            sprite_dimensions_in_pixels: [64., 64.],
        };
        let asteroid_physics = |centre_position: Vector2<f32>| Physics {
            centre_position,
            bounding_dimensions: vec2(64., 64.),
            velocity: vec2(0., 0.),
            facing: vec2(0., 1.),
        };
        {
            let _add_asteroid = |centre_position| {
                let id = game_state.entity_id_allocator.allocate();
                game_state.graphics.insert(id, asteroid_graphics.clone());
                game_state
                    .physics
                    .insert(id, asteroid_physics(centre_position));
            };
        }
        game_state
    }
    pub fn to_render(&self) -> impl Iterator<Item = ToRender> {
        self.physics.iter().filter_map(move |(id, physics)| {
            self.graphics
                .get(id)
                .map(|graphics| ToRender { physics, graphics })
        })
    }
    pub fn update(&mut self, input_model: &InputModel) {
        for physics in self.physics.values_mut() {
            physics.centre_position += physics.velocity;
        }

        if let Some(physics) = self.physics.get_mut(&self.player_id) {
            if let Some(aim_vector) = input_model.aim_vector() {
                physics.facing = aim_vector;
            }
            const THRUST_MULTIPLIER: f32 = 0.2;
            const VELOCITY_MAX_MAGNITUDE: f32 = 10.;
            const VELOCITY_MAX_MAGNITUDE2: f32 =
                VELOCITY_MAX_MAGNITUDE * VELOCITY_MAX_MAGNITUDE;
            let mut next_velocity = physics.velocity
                + physics.facing * input_model.thrust * THRUST_MULTIPLIER;
            physics.velocity = if next_velocity.magnitude2() > VELOCITY_MAX_MAGNITUDE2 {
                next_velocity.normalize() * VELOCITY_MAX_MAGNITUDE
            } else {
                next_velocity
            };
        }
    }
}