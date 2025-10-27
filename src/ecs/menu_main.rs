use bevy::{
    audio::Volume,
    ecs::{component::HookContext, entity_disabling::Disabled, world::DeferredWorld},
    prelude::*,
    scene::SceneInstanceReady,
    text::cosmic_text::Edit,
};
use bevy_flair::style::components::NodeStyleSheet;
use bevy_simple_text_input::TextInput;
use lightyear::prelude::Connected;

use crate::{
    ecs::cameras::MainCamera,
    scenes::LoadScene,
    systems::{
        game::{Game, SceneMetadata},
        network::NetworkCrap,
        scenes::OnSpawnScene,
    },
    util::parts::PartOf,
};

// TODO: parallax on menu items

/// top level marker for Main Menu
#[derive(Debug, Default, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct MenuMain;

/// test to see if inspector needs reflect(Component) (answer: it don't)
#[derive(Debug, Default, Clone, Component, Reflect)]
pub struct SillyTest(pub usize);

/// a camera in blender which the MainCamera should be moved too
#[derive(Debug, Default, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct BlenderCamera;
impl BlenderCamera {
    fn on_add(
        query: Query<(Entity, &GlobalTransform), Added<BlenderCamera>>,
        camera: Query<Entity, With<MainCamera>>,
        mut commands: Commands,
    ) {
        let Ok(camera) = camera.single() else { return };
        for (entity, gt) in query {
            commands.entity(camera).insert((
                gt.compute_transform(),
                gt.clone(), // also insert gt so that first frame is correct
            ));
            // TODO better handle non-main cameras
            commands.entity(entity).remove::<Camera>();
        }
    }
}

#[derive(Debug, Event, Reflect)]
pub struct OnSubmitDumb(String);
impl OnSubmitDumb {
    fn plugin(app: &mut App) {
        // convert event to trigger
        fn submit(
            mut events: EventReader<bevy_ui_text_input::TextSubmitEvent>,
            mut commands: Commands,
        ) {
            for event in events.read() {
                commands
                    .entity(event.entity)
                    .trigger(OnSubmitDumb(event.text.clone()));
            }
        }
        app.add_systems(PostUpdate, submit);
        app.add_observer(despawn);
        app.register_type::<OnSubmitDumb>();
        app.register_type::<DefaultFloor>();
    }
}

pub fn plugin(app: &mut App) {
    app.register_type::<BlenderCamera>();
    app.register_type::<MenuMain>();
    app.register_type::<SillyTest>();
    app.add_systems(
        PostUpdate,
        BlenderCamera::on_add.after(TransformSystem::TransformPropagate),
    );
    app.add_observer(on_create_lobby);
    app.add_observer(on_join_lobby);
    app.add_plugins(OnSubmitDumb::plugin);
}

pub fn spawn_main_menu(
    asset_server: Res<AssetServer>,
    mut commands: Commands,

    nc: Res<NetworkCrap>,
) {
    let mut root = commands.spawn((
        MenuMain,
        Node::default(),
        SillyTest(100),
        NodeStyleSheet::new(asset_server.load("menu/main_menu.css")),
    ));

    root.with_children(|root| {
        root.spawn((
            Name::new("logo"),
            ImageNode::new(asset_server.load("menu/main_logo.png")),
        ));

        root.spawn((
            Button,
            Name::new("create"),
            children![Text::new("create lobby")],
        ))
        .observe(|_: Trigger<Pointer<Click>>, mut commands: Commands| {
            commands.trigger(CreateLobby);
        });
        root.spawn((Node::default(), Name::new("join")))
            .with_children(|parent| {
                parent.spawn((Label, Text::new("join lobby")));

                // this sucks https://github.com/ickshonpe/bevy_ui_text_input/issues/28
                let mut input_buffer = bevy_ui_text_input::TextInputBuffer::default();
                let editor = &mut input_buffer.editor;
                editor.insert_string(&nc.address.to_string(), None);

                parent
                    .spawn((
                        bevy_ui_text_input::TextInputNode {
                            mode: bevy_ui_text_input::TextInputMode::SingleLine,
                            ..default()
                        },
                        input_buffer,
                        Node {
                            width: Val::Vw(40.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba_u8(255, 255, 255, 100)),
                    ))
                    .observe(|val: Trigger<OnSubmitDumb>, mut commands: Commands| {
                        // TODO blink bg color
                        commands.trigger(JoinLobby {
                            addr: val.0.clone(),
                        });
                    });
            });
        root.spawn((Button, Name::new("exit"), children![Text::new("exit")]))
            .observe(|_: Trigger<Pointer<Click>>, mut cmd: Commands| {
                cmd.send_event(AppExit::Success);
            });

        root.spawn((
            AudioPlayer::<AudioSource>(asset_server.load("sounds/corn_game.ogg")),
            PlaybackSettings {
                mode: bevy::audio::PlaybackMode::Despawn,
                volume: Volume::Linear(1.0),
                ..Default::default()
            },
        ));
    });

    let root = root.id();

    commands.spawn((LoadScene::new("scenes/main_menu.gltf"), PartOf(root)));
}

#[derive(Debug, Clone, Event, Reflect)]
struct CreateLobby;

fn on_create_lobby(
    _: Trigger<CreateLobby>,
    menu: Query<Entity, With<MenuMain>>,
    mut commands: Commands,
) -> Result {
    //XXX observer never despawned
    commands.add_observer(|_: Trigger<OnAdd, Connected>, mut commands: Commands| {
        // TODO, server should be in charge of this
        // commands.trigger(LoadLobby);
    });

    #[cfg(not(target_family = "wasm"))]
    commands.run_system_cached(crate::systems::network::start_server);
    commands.run_system_cached(crate::systems::network::start_client);
    commands.spawn((Game::lobby(),));

    Ok(())
}

#[derive(Debug, Clone, Event, Reflect)]
struct JoinLobby {
    addr: String,
}

fn on_join_lobby(
    _: Trigger<JoinLobby>,
    menu: Query<Entity, With<MenuMain>>,
    mut commands: Commands,
) -> Result {
    commands.add_observer(|_: Trigger<OnAdd, Connected>, mut commands: Commands| {
        // TODO, server should be in charge of this
        // commands.trigger(LoadLobby);
    });

    #[cfg(not(target_family = "wasm"))]
    commands.run_system_cached(crate::systems::network::start_client);

    Ok(())
}

/// spawn lobby scene, wait for it to be ready, then despawn menu
// #[derive(Debug, Clone, Event, Reflect)]
// struct LoadLobby;

// fn on_load_lobby(
//     _: Trigger<LoadLobby>,
//     menu: Query<Entity, With<TopLevelGameStateEntity>>,
//     mut commands: Commands,
// ) -> Result {
//     let menu = menu.single()?;

//     commands
//         .spawn((LoadScene::new("scenes/lobby.glb"),))
//         .with_children(|parent| {
//             // temp floor, DELETEME
//             parent.spawn((
//                 Name::from("blah Floor"),
//                 Transform::from_scale(Vec3::new(1000.0, 0.0, 1000.0)),
//                 avian3d::prelude::Collider::cuboid(1.0, 0.1, 1.0),
//                 avian3d::prelude::RigidBody::Static,
//             ));
//         })
//         .observe(
//             move |_: Trigger<SceneInstanceReady>, mut commands: Commands| {
//                 commands.entity(menu).despawn();
//             },
//         );

//     Ok(())
// }

/// menu needs to despawn or close after client connects to server and server spawns a level
fn despawn(
    trigger: Trigger<OnAdd, SceneMetadata>,
    menu: Single<Entity, With<MenuMain>>,
    mut commands: Commands,
) {
    commands.entity(*menu).despawn();
}

#[derive(Debug, Clone, Copy, Component, Reflect)]
#[reflect(Component)]
#[component(on_add = default_floor)]
pub struct DefaultFloor;
fn default_floor(mut world: DeferredWorld, context: HookContext) {
    world.commands().entity(context.entity).insert((
        Name::from("blah Floor"),
        Transform::from_scale(Vec3::new(1000.0, 0.0, 1000.0)),
        avian3d::prelude::Collider::cuboid(1.0, 0.1, 1.0),
        avian3d::prelude::RigidBody::Static,
    ));
}
