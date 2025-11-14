// diagetic ui

use std::collections::HashMap;
use std::ops::DerefMut;

use bevy::ecs::relationship::{Relationship, RelationshipSourceCollection};
use bevy::picking::pointer::{PointerId, PointerInput, PointerLocation};
use bevy::prelude::*;
// diagetic ui

use bevy::prelude::*;
use bevy::render::camera::{NormalizedRenderTarget, RenderTarget};
use bevy::render::view::RenderLayers;
use bevy::text::FontSmoothing;
use bevy_lunex::prelude::*;
use uuid::Uuid;

use crate::ecs::menu_crt_effect::PostProcessSettings;
use crate::ecs::test_cube::TestCube;
use crate::scenes::resolver::{EntityPointer, EntityResolver};
use crate::scenes::LoadScene;
use crate::systems::animation_context::{AnimationContext, AutoPlayAnimation};
use crate::systems::camera_target::{CameraFocus, CameraPositionTarget};
use crate::systems::game::{Level, SwapLevel};
use crate::systems::interactions::{InteractionText, Interactable, Interaction};
use crate::util::parts::{Rel, RelOf, RelPlugin};
use crate::util::propogate::Propagate;

// 1. show ui on computer
// - ui only test scene
// - pane with marker component
// - interact can just be on the pane.
// 2. interact -> mouse / cursor mode (picking)
// - mouse has relation to the pane
// 3. networking
// 4. level load

// make it all hard coded, make diagenic ui abstraction later.
// use lunex?

// rebuild ui in code, there isn't really a good way to do it as an asset

pub fn plugin(app: &mut App) {
    app.register_type::<SpawnComputerMenu>();
    app.register_type::<ComputerMenuUi>();
    app.register_type::<Cursor>();
    app.register_type::<Mouse>();
    app.register_type::<BlenderMouse>();
    app.add_plugins((
        RelPlugin::<Cursor>::default(),
        RelPlugin::<Mouse>::default(),
        GuysPlugin,
    ));
    app.add_observer(spawn_diagetic_interface);
    app.add_systems(Update, (Cursor::move_update, Cursor::escape));
    app.add_systems(PostUpdate, BlenderMouse::on_add);

    // unique render layers start at 11000
    app.insert_resource(RenderLayersAlloc(10));
}

/// Resource to hand out unique RenderLayers for seperate renderings
/// simple counter
#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct RenderLayersAlloc(usize);

impl RenderLayersAlloc {
    pub fn get(&mut self) -> RenderLayers {
        self.0 += 1;
        RenderLayers::none().with(self.0)
    }
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct SpawnComputerMenu;

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct ComputerMenuUi {
    pub texture: Handle<Image>,
    pub active: bool,
    pub pointer: Entity,
}

/// see: https://github.com/bytestring-net/bevy_lunex/blob/main/examples/dualcamera/src/main.rs
pub fn spawn_diagetic_interface(
    trigger: Trigger<OnAdd, SpawnComputerMenu>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut layers: ResMut<RenderLayersAlloc>,
    server: ResMut<AssetServer>,
) {
    // STEP 1: create render texture for this ui.
    let canvas = images.add(<Image as bevy_lunex::ImageTextureConstructor>::clear_render_texture());

    // STEP 2: spawn ui root, with plane to draw the canvas
    let plane = Plane3d {
        normal: Dir3::Y,
        half_size: Vec2::new(1.01, 1.01),
    };

    // must be seperate from Node bc of pointer debug plugin setting visibility
    let pointer = commands.spawn(
        PointerId::Custom(Uuid::new_v4())
    ).id();

    // spawn the ui root.
    // let root_entity = commands
    //     .spawn((
    let root_entity = commands
        .entity(trigger.target())
        .insert((
            ComputerMenuUi {
                texture: canvas.clone(),
                active: false,
                pointer
            },
            Mesh3d(meshes.add(plane)),
            MeshMaterial3d(materials.add(StandardMaterial {
                unlit: true,
                base_color_texture: Some(canvas.clone()),
                // base_color: Color::BLACK,
                //emissive_texture: Some(canvas.clone()),
                //emissive_exposure_weight: 1.0,
                ..default()
            })),
            Interactable::default(),
            InteractionText {
                string: "use".into(),
                show: "use".into(),
            },
            children![(
                Name::new("camera target"),
                // magic value, tuned with inspector
                Transform::from_xyz(0.0, 4.5, 0.3).looking_at(Vec3::Z * 0.4, -Vec3::Z),
                CameraPositionTarget::default(),
            )], //Name::new("Canvas"),//XXX breaks name pointer in blende
        ))
        .observe(
            |trigger: Trigger<Interaction>,
             mut query: Query<(&mut ComputerMenuUi, &mut Interactable)>,
             mut commands: Commands| {
                let (mut menu, mut interact) = query.get_mut(trigger.target())?;
                menu.active = true;
                interact.active = true;

                commands.entity(trigger.target()).insert(CameraFocus);

                Ok(())
            },
        )
        .id();

    // STEP 3: create a orthographic camera for the ui only
    let render_layers = layers.get();

    // commands.spawn((
    //     Camera3d::default(),
    //     // camera renders to image texture
    //     Camera {
    //         target: RenderTarget::Image(canvas.clone().into()),
    //         clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)),
    //         order: -1, //?
    //         ..default()
    //     },
    //     // camera will go from -1 to 1 in x and y
    //     // TODO 0 to 1
    //     Projection::from(OrthographicProjection {
    //         scaling_mode: bevy::render::camera::ScalingMode::Fixed {
    //             width: 2.0,
    //             height: 2.0,
    //         },
    //         scale: 1.0,
    //         ..OrthographicProjection::default_3d()
    //     }),

    //     // crt postprocessing effect
    //     PostProcessSettings{
    //         ..Default::default()
    //     },

    //     Transform::from_xyz(1.0, 0.0, 0.0).looking_at(Vec3::ZERO, Dir3::Y),
    //     render_layers.clone(),
    //     // ChildOf(root_entity),
    // ));

    let camera_ui = commands
        .spawn((
            Camera2d,
            Camera {
                target: RenderTarget::Image(canvas.clone().into()),
                clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 1.0)),
                order: -2, //?
                ..default()
            },
            Name::from("Menu UI Camera"),
            render_layers.clone(),
            PostProcessSettings {
                ..Default::default()
            },
            // ChildOf(root_entity),
        ))
        .id();

    // STEP 4: spawn the ui. it must be spawned at root so as not to have transform propogation.
    // but we can give it a relation so despawn still works
    // commands.spawn((
    //     // LoadScene::new("scenes/menu.glb"),
    //     Transform::default(),
    //     Propagate(render_layers.clone()),
    //     children![
    //         Sprite::from_image(server.load("menu/wallpaper.jpg")),
    //     ],
    // ));

    const YELLOW_BRIGHT: Color = Color::srgb_u8(0x67, 0x60, 0x05); // toolbar
    const YELLOW_DARK: Color = Color::srgb_u8(0x42, 0x3d, 0x00); // toolbar

    let font = server.load("fonts/roboto.ttf");

    commands
        .spawn((
            Node {
                width: Val::Vw(100.0),
                height: Val::Vh(100.0),
                ..default()
            },
            UiTargetCamera(camera_ui),
            Propagate(render_layers.clone()),
        ))
        .with_children(|parent| {
            parent.spawn((
                Name::new("wallpaper"),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Vw(100.0),
                    height: Val::Vh(100.0),
                    ..default()
                },
                ImageNode::new(server.load("menu/wallpaper.jpg")),
                ZIndex(-10),
            ));
            parent
                .spawn((
                    Name::new("toolbar"),
                    Node {
                        position_type: PositionType::Absolute,
                        box_sizing: BoxSizing::ContentBox,
                        width: Val::Percent(100.0),
                        height: Val::Px(23.0),
                        border: UiRect::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(YELLOW_BRIGHT),
                    BorderColor(Color::BLACK),
                ))
                .with_children(|toolbar| {
                    toolbar.spawn((
                        Name::new("logo"),
                        ImageNode::new(server.load("menu/logo.png")),
                        Node {
                            height: Val::Percent(100.0),
                            ..default()
                        },
                    ));
                    toolbar.spawn((
                        Name::new("spacer"),
                        Node {
                            flex_grow: 1.0,
                            border: UiRect::left(Val::Px(4.0)),
                            ..default()
                        },
                        BorderColor(Color::BLACK),
                    ));
                    toolbar.spawn((
                        Name::new("clock"),
                        Node {
                            margin: UiRect {
                                top: Val::Px(2.0),
                                right: Val::Px(10.0),
                                ..default()
                            },
                            ..default()
                        },
                        Text::new("12:57"),
                        TextFont {
                            font: font.clone(),
                            font_size: 16.0,
                            line_height: default(),
                            font_smoothing: FontSmoothing::None,
                        },
                        TextColor(Color::BLACK),
                    ));
                });
            parent
                .spawn((
                    Name::new("window"),
                    Node {
                        position_type: PositionType::Relative,
                        width: Val::Percent(80.0),
                        height: Val::Percent(70.0),
                        left: Val::Percent(13.0),
                        top: Val::Percent(17.0),
                        border: UiRect::all(Val::Px(5.0)),
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                    BackgroundColor(YELLOW_DARK),
                    BorderColor(Color::BLACK),
                ))
                .with_children(|window| {
                    window
                        .spawn((
                            Name::new("windowbar"),
                            Node {
                                box_sizing: BoxSizing::ContentBox,
                                width: Val::Percent(100.0),
                                height: Val::Px(16.0),
                                border: UiRect::bottom(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(Color::BLACK),
                            BorderColor(Color::BLACK),
                        ))
                        .with_children(|windowbar| {
                            windowbar.spawn((
                                Name::new("name"),
                                Text::new("CORN GAME"),
                                TextFont {
                                    font: font.clone(),
                                    font_size: 16.0,
                                    line_height: default(),
                                    font_smoothing: FontSmoothing::None,
                                },
                                TextColor(Color::BLACK),
                                BackgroundColor(YELLOW_BRIGHT),
                            ));
                            windowbar.spawn((
                                Name::new("spacer"),
                                Node {
                                    flex_grow: 1.0,
                                    ..default()
                                },
                            ));
                            windowbar.spawn((
                                Name::new("minus"),
                                Node { ..default() },
                                ImageNode::new(server.load("menu/minus.jpg")),
                            ));
                            windowbar.spawn((
                                Name::new("plus"),
                                Node { ..default() },
                                ImageNode::new(server.load("menu/plus.jpg")),
                            ));
                        });
                    window
                        .spawn((
                            Name::new("grid"),
                            Node {
                                display: Display::Grid,
                                ..default()
                            },
                        ))
                        .with_children(|grid| {
                            grid.spawn((
                                ImageNode::new(server.load("menu/lock.png")),
                                Node {
                                    width: Val::Px(100.0),
                                    height: Val::Px(100.0),
                                    ..default()
                                },
                            ));
                        });
                    window.spawn((
                        Name::new("guys"),
                        Node {
                            position_type: PositionType::Absolute,
                            bottom: Val::Px(18.0),
                            left: Val::Px(12.0),
                            width: Val::Px(155.0),
                            ..default()
                        },
                        Text::new("XXX"),
                        Guys,
                    ));
                    window
                        .spawn((
                            Name::new("start"),
                            ImageNode::new(server.load("menu/start.png")),
                            Node {
                                position_type: PositionType::Absolute,
                                bottom: Val::Px(18.0),
                                right: Val::Px(12.0),
                                width: Val::Px(155.0),
                                ..default()
                            },
                        ))
                        .observe(|_: Trigger<Pointer<Released>>, mut commands: Commands| {
                            // spawn the level
                            commands.trigger(SwapLevel {
                                level: Level {
                                    id: "level1".to_string(),
                                    scenes: HashMap::from(
                                        [(
                                            "scene1".to_string(),
                                            "scenes/cornmenu_min.glb".to_string(),
                                        )], // testing out reflection strings, we want to do a cli thing
                                    ),
                                },
                            });
                        });
                });

            let size = Vec2::new(512.0, 512.0);
            parent.spawn((
                Name::new("cursor"),
                RelOf::<Cursor>::new(root_entity),
                Cursor {
                    speed: 0.4,
                    pos: size * 0.65,
                    bounds: size,
                },
                ImageNode::new(server.load("menu/cursor.png")),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(47.0),
                    // left: Val::Percent(50.0),
                    // top: Val::Percent(50.0),
                    ..default()
                },
                Pickable::IGNORE, //NOTE: cursor must not itself be pickable
                ZIndex(10)
            ));
        });
}

fn image() {}

fn inner_computer_menu(mut commands: Commands) {
    // background
    // commands.spawn();

    // window
    // inner window
    // players

    // toolbar
    // cursor
}

// things we want
// - spinning players
//      - requires seperate render layer
//      - have to propogate
// - cursor
//      - requires picking cursor, and pausing normal camera controls
// - hover and click
//      - requires regular picking stuff
// - crt filter ???

#[derive(Debug, Clone, Component, Default, Reflect)]
#[reflect(Component)]
struct Cursor {
    speed: f32,
    pos: Vec2,
    bounds: Vec2,
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
struct BlenderMouse {
    target: EntityPointer,
}

impl BlenderMouse {
    /// adds Mouse with bounds based on transform
    /// and RelOf<Mouse> pointing to ComputerMenuUi root
    /// NOTE didn't work as observer because SceneRoot not attached yet, mouse had no parent
    fn on_add(
        // trigger: Trigger<OnAdd, Self>,
        query: Query<(Entity, &Self), Added<Self>>,
        transform: Query<&Transform>,
        resolver: EntityResolver,
        mut commands: Commands,
    ) -> Result {
        // let entity = trigger.target();
        // let c = query.get(entity)?;
        for (entity, c) in query.iter() {
            let menu_entity = resolver.resolve(entity, &c.target)?;
            let mouse_entity = resolver.parents.get(entity)?.0;

            let t = transform.get(entity)?;
            let center = t.translation.xz() + transform.get(mouse_entity)?.translation.xz();
            let scale = t.scale.xz();

            let mouse = Mouse {
                bounds: Rect::from_center_half_size(center, scale),
            };

            commands
                .entity(mouse_entity)
                .insert((RelOf::<Mouse>::new(menu_entity), mouse));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Component, Default, Reflect)]
#[reflect(Component)]
struct Mouse {
    bounds: Rect,
}

impl Cursor {
    fn escape(
        keys: Res<ButtonInput<KeyCode>>,
        mut menu: Query<(Entity, &mut ComputerMenuUi, &mut Interactable)>,
        mut commands: Commands,
    ) -> Result {
        let Ok((entity, mut menu, mut interact)) = menu.single_mut() else {
            return Ok(());
        };
        if !menu.active {
            return Ok(());
        }
        if keys.just_pressed(KeyCode::Escape) {
            menu.active = false;
            interact.active = false;
            commands.entity(entity).remove::<CameraFocus>();
        }
        Ok(())
    }

    fn move_update(
        mut menu: Query<(&ComputerMenuUi, &Rel<Cursor>, &Rel<Mouse>)>,
        mut pointer: Query<(&PointerId, &mut PointerLocation)>,
        mut cursor: Query<(&mut Cursor, &mut Node)>,
        mut mouse: Query<(&Mouse, &mut Transform), Without<Cursor>>,
        
        input: Res<bevy::input::mouse::AccumulatedMouseMotion>,
        button: Res<ButtonInput<MouseButton>>,

        mut commands: Commands,    
        mut pointer_inputs: EventReader<PointerInput>,

    ) -> Result<(), BevyError> {
        let Ok((menu, cursor_entity, mouse_entity)) = menu.single_mut() else {
            return Ok(());
        };
        let ( pointer_id, mut pointer_location) = pointer.get_mut(menu.pointer)?;

        if !menu.active {
            return Ok(());
        }

        let cursor_entity = cursor_entity.iter().next().unwrap();
        let mouse_entity = mouse_entity.iter().next().unwrap();

        // CURSOR
        let (mut cursor, mut node) = cursor.get_mut(cursor_entity)?;

        let a = cursor.pos + input.delta * cursor.speed;
        let a = a.clamp(Vec2::ZERO, cursor.bounds);
        cursor.pos = a;

        node.top = Val::Px(cursor.pos.y);
        node.left = Val::Px(cursor.pos.x);

        // MOUSE
        let (mouse, mut transform) = mouse.get_mut(mouse_entity)?;
        let ratio = cursor.pos / cursor.bounds;

        let p = mouse.bounds.min + mouse.bounds.size() * ratio;
        transform.translation = Vec3::new(p.x, 0.0, p.y);

        // PICKING
        let position = a;
        let location = bevy::picking::pointer::Location {
            position,
            target: NormalizedRenderTarget::Image(menu.texture.clone().into()),
        };
        pointer_location.location = Some(location.clone());

        for input in pointer_inputs
            .read()
            // .filter(|input| &input.pointer_id == pick_pointer_id)
        {
            commands.send_event(PointerInput {
                location: location.clone(),
                pointer_id: *pointer_id,
                action: input.action,
            });
        }

        Ok(())
    }
}

#[derive(Debug, Reflect, Component)]
struct Guys;

struct GuysPlugin;
impl Plugin for GuysPlugin {
    fn build(&self, app: &mut App) {
        fn update(
            state: Single<&crate::systems::network::NetworkState>,
            guys: Query<(&Guys, &mut Text)>,
        ) {
            for (_, mut text) in guys {
                text.0 = state.players.len().to_string();
            }
        }
        app.add_systems(Update, update);
        app.register_type::<Guys>();
    }
}
