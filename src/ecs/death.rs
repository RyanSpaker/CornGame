use avian3d::prelude::{LinearVelocity, LockedAxes, Position, RigidBody, Sensor};
use bevy::{
    ecs::observer,
    input::mouse,
    math::FloatPow,
    prelude::*,
    state::commands,
    window::CursorOptions,
};
use bevy_dog::settings::DoGSettings;
use bevy_easings::CustomComponentEase;
use bevy_enhanced_input::prelude::*;
use bevy_tnua::util::VelocityBoundary;
use lightyear::prelude::AppComponentExt;
use wgpu::hal::auxil::db;

use crate::{
    ecs::{
        cameras::MainCamera,
        corn::{self, sensor::GradientTestSettings},
    },
    scenes::{LoadScene, main_menu},
    systems::{
        animation_context::{AnimationContext, AutoPlayAnimation},
        camera_target::{ControlledBy, Controls},
        character::{Character, Player, SpawnPlayerEvent, controller::CornGameCharController},
        network::ReplicateAuto,
    },
    util::{math::{ExpEaseTo, lerp}, specialized_material::SetItemPipelineDbg},
};

pub fn plugin(app: &mut App) {
    app.add_plugins(DeathCharController::plugin);

    app.register_type::<Alive>();
    app.register_type::<DeathSeq>();
    app.register_type::<Dead>();
    app.register_type::<DeadBodyOf>();
    app.register_type::<DeadBody>();
    app.register_type::<DeathCharController>();

    // Only alive and dead are network synced
    app.register_component::<Alive>();
    app.register_component::<Dead>();

    app.add_systems(Startup, crate::Cli::runnable(spawn_dead));

    // on alive
    app.add_systems(
        Update,
        |player: Query<(Entity, Has<Player>), (Added<Alive>, With<Character>)>,
         main_camera: Query<Entity, With<MainCamera>>,
         mut commands: Commands| {
            for (entity, is_local_player) in player {
                if is_local_player {
                    // activate normal character controller
                    commands
                        .entity(entity)
                        .insert_if_new(CornGameCharController::default())
                        .remove::<DeathCharController>();

                    // activate normal camera
                    if let Some(camera) = main_camera.iter().next() {
                        commands.entity(camera).remove::<DoGSettings>();
                    }
                } else {
                    // nothing for now, this is handled by an event instead
                }
            }
        },
    );

    // On death
    app.add_systems(
        Update,
        |player: Query<(Entity, Has<Player>), (Added<Dead>, With<Character>)>,
         main_camera: Query<Entity, With<MainCamera>>,
         mut commands: Commands,
         clear: Res<ClearColor>,
         asset_server: ResMut<AssetServer>| {
            for (entity, is_local_player) in player {
                commands
                    .entity(entity)
                    .remove::<avian3d::prelude::RigidBody>()
                    .despawn_related::<Children>(); // remove colliders

                // TODO temp spawn a transparent sphere as orb placeholder
                commands.entity(entity).with_children(|parent| {
                    parent.spawn((
                        Mesh3d::from(
                            asset_server.add(
                                Mesh::from(Tetrahedron::default()).scaled_by(Vec3::splat(0.15)),
                            ),
                        ),
                        MeshMaterial3d::from(asset_server.add(StandardMaterial {
                            base_color: Color::srgba(0.2, 0.5, 1.0, 0.3),
                            unlit: true,
                            ..default()
                        })),
                    ));
                });

                if is_local_player {
                    // activate death character controller
                    // note, could have it be a relation to a character controller, its a 1 to 1
                    commands
                        .entity(entity)
                        .remove::<CornGameCharController>()
                        .remove::<bevy_tnua::prelude::TnuaController>(); // TODO remove all the tnua crap? perhaps replace entire entity in place
                    commands
                        .entity(entity)
                        .insert_if_new(DeathCharController::bundle());

                    // activate death camera
                    if let Some(camera) = main_camera.iter().next() {
                        commands
                            .entity(camera)
                            .insert_if_new((
                                bevy_dog::settings::DoGSettings {
                                    dog_strength: -10.0,
                                    enable_layers: Vec4::splat(1.0),
                                    thresholds: Vec4::new(80.0, 40.0, 20.0, 1.0),
                                    thresholding: 1,
                                    ..bevy_dog::settings::DoGSettings::OUTLINE_DITHER
                                },
                                bevy_dog::settings::PassesSettings::default(),
                            ))
                            .insert((
                                AmbientLight {
                                    color: Color::WHITE,
                                    brightness: 1000.0,
                                    ..default()
                                },
                                DistanceFog {
                                    color: clear.0, //Color::srgb(0.25, 0.25, 0.25),
                                    falloff: FogFalloff::Linear {
                                        start: 4.0,
                                        end: 22.0,
                                    },
                                    ..default()
                                },
                            ))
                            .insert(ControlledBy(entity));
                    }

                    // TODO can there be issues with one frame flicker when death seq ends? Perhaps this should be an observer?
                } else {
                }
            }
        },
    );

    // death sequence
    app.add_systems(
        Update,
        |this: Single<(Entity, &mut DeathSeq)>, mut commands: Commands, time: Res<Time>| {
            let (entity, mut this) = this.into_inner();
            this.timer.tick(time.delta());
            if this.timer.just_finished() {
                // RATIONAL: death_seq moves player, and puts him back at the end.
                // TODO might just be better to spawn a new entity and attach the camera to that.
                commands
                    .entity(entity)
                    .insert(Dead)
                    .insert(Position::from(
                        this.death_position.unwrap_or_default().translation,
                    )) // TODO rotation?
                    .remove::<DeathSeq>();
                if let Some(field) = this.death_field {
                    commands.entity(field).despawn();
                }
            }
        },
    );
    app.add_observer(
        |trigger: Trigger<OnAdd, DeathSeq>,
         mut player: Query<&mut DeathSeq>,
         mut commands: Commands,
         asset_server: Res<AssetServer>| {
            // RATIONAL: DeathSeq owns the visual death transition. Spawning, and setting up camera belongs here.
            // TODO test now with black screen.

            // black screen TODO blood animation https://github.com/bevyengine/bevy/issues/5221
            // bevy fullscreen ui image node
            let black_screen = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    ImageNode::new(asset_server.load("textures/black_screen_death.jpg")),
                ))
                .id();

            let mut death_seq = player.get_mut(trigger.target()).unwrap();
            death_seq.death_field = Some(black_screen); // so it get's despawned.

            // spooky audio
            commands.spawn((
                AudioPlayer::new(asset_server.load("sounds/spooky.ogg")),
                PlaybackSettings {
                    mode: bevy::audio::PlaybackMode::Despawn,
                    volume: bevy::audio::Volume::Linear(0.1),
                    paused: false,
                    ..Default::default()
                },
            ));
        },
    );
}

#[derive(Debug, Clone, Component, Reflect, serde::Serialize, serde::Deserialize, PartialEq)]
#[reflect(Component)]
pub enum Alive {
    // ignored by mobs
    Dev,
    // cannot die
    Immortal,
    // standard
    Normal,
}

#[derive(Debug, Clone, Component, Reflect, serde::Serialize, serde::Deserialize, PartialEq)]
#[reflect(Component)]
pub struct Dead;

/// command used to kill player
pub struct Die {
    entity: Entity,
}
impl Die {
    pub fn new(entity: Entity) -> Self {
        Self { entity }
    }
}

// XXX: per entity behavior, might be better to use an event.
impl Command for Die {
    fn apply(self, world: &mut World) {
        // TODO if not player...

        let Some(alive) = world.entity(self.entity).get::<Alive>() else {
            // already dead
            warn!(
                "Entity {:?} is already dead, cannot die again.",
                self.entity
            );
            return;
        };
        if alive != &Alive::Normal {
            // cannot die
            return;
        }

        // Trigger the death sequence
        let pos = world
            .get::<GlobalTransform>(self.entity)
            .map(|p| p.compute_transform());
        world.entity_mut(self.entity).insert(DeathSeq {
            timer: Timer::from_seconds(3.0, TimerMode::Once),
            death_position: pos,
            death_field: None,
        });

        // TODO when death sequence is refactored to use seperate camera
        world.entity_mut(self.entity).remove::<Alive>(); //.insert(Dead);

        let mut pos = pos.unwrap();
        pos.translation.y = 0.0; // annoying, player is not bottom centered, but dead body is

        // spawn dead body. for now only on client.
        // TODO fall animation... would require copying animation state. might be better to just move the player model to the dead body.
        // TODO Networking options. replicate or event or reuse model, or have die be replicated event
        // first get working as replicated spawn
        // - then refactor as co-spawn with id
        // - then refactor as reuse model
        // - then make die a replicate event
        let _dead_body = world.spawn((
            ReplicateAuto,
            pos,
            // Visuals, etc
            DeadBodyOf(self.entity),
            LoadScene::new("models/mixamo.glb#dead"),
            AutoPlayAnimation { repeat: false },
            AnimationContext::default(),
        ));
    }
}

/// IDEA this should be its own entity, with relation to player. and death field as child, instead of component on player.
/// this additionally makes sense since different death sequences can be different entities.
///
/// 1. player is teleported to a location around the perimeter of the field such that their look direction is the same
///     - This is so that if two players die at the same time, they see each other.
/// 2. player character controller remains active, it is merely locked forward (to keep footsteps working)
///     - we know we want the sequence to feel like walking
/// 3. death field is has offsets so that there is no visual change when you are teleported to the corn
///     1. since player is moved, this requires a xz offset
///     2. all random variables in shader must be worldspace xz based (wind and corn offset)
/// 4. any nearby carving should be copied to the carving for the death field
#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct DeathSeq {
    timer: Timer,
    death_position: Option<Transform>,
    death_field: Option<Entity>,
}

// alternative death sequence ideas (for monster and/or death when not in corn)
// - screen goes black
// - black sphere / nearfield closes in on camera
// - fall to knees and ground?
// - wind gets really loud as monster nears, screen goes red tint and silent/ringing once he gets you.

/// relationship marks the corpse of the player
#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[relationship(relationship_target = DeadBody)]
pub struct DeadBodyOf(Entity);

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[relationship_target(relationship = DeadBodyOf)]
pub struct DeadBody(Vec<Entity>);

/// death character controller
#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct DeathCharController {
    height: f32,
    height_drop: f32,

    /// exp rate for height changes (up, down)
    height_rate: (f32, f32),

    /// whether to depress at edges or not.
    use_real_vel_for_drop: bool,

    speed: f32,
    portals: bool,

    /// 1.0 means offset contains the edge
    gradient_threshold: f32,

    /// sample offset used when testing corn gradient
    gradient_offset: f32,

    /// natural fov when not near edge
    fov_natural: f32,

    /// amount to narrow fov when pushing against edge (zoom effect)
    fov_narrowing: f32,

    /// exp rate for fov changes (in, out)
    fov_rate: (f32, f32),

    // used internally
    movement: Vec3,
}

impl Default for DeathCharController {
    fn default() -> Self {
        Self {
            height: 2.8,
            height_drop: 0.5,
            height_rate: (2.5, 5.0),
            use_real_vel_for_drop: true,
            speed: 5.0,
            portals: true,
            gradient_threshold: 0.05,
            gradient_offset: 0.5, // if this is 0.25 it gets bumpy
            fov_natural: 60.0,
            fov_narrowing: 15.0,
            fov_rate: (1.0, 3.0),

            movement: Vec3::ZERO, // internal
        }
    }
}

#[derive(InputAction)]
#[action_output(Vec2)]
struct Movement;

#[derive(InputAction)]
#[action_output(Vec2)]
struct Rotate;

impl DeathCharController {
    // TODO add fields for controlling movement speed, etc
    pub fn plugin(mut app: &mut App) {
        app.add_systems(
            FixedUpdate,
            Self::update.before(avian3d::prelude::PhysicsSet::Prepare),
        );
        app.add_input_context::<DeathCharController>();
    }

    pub fn bundle() -> impl Bundle {
        (
            DeathCharController::default(),
            // RigidBody::Kinematic,
            // LockedAxes::ROTATION_LOCKED.lock_translation_y(),
            Sensor, // we don't want physics collisions for ghosts! XXX make sure there are no child colliders
            actions!(DeathCharController[
                (
                    Action::<Movement>::new(),
                    DeadZone::default(), // Apply non-uniform normalization that works for both digital and analog inputs, otherwise diagonal movement will be faster.
                    SmoothNudge::default(), // Make movement smooth and independent of the framerate. To only make it framerate-independent, use `DeltaScale`.
                    Bindings::spawn((
                        Cardinal::wasd_keys(),
                        Axial::left_stick(),
                    ))
                ),
                (
                    Action::<Rotate>::new(),
                    Bindings::spawn((
                        // Bevy requires single entities to be wrapped in `Spawn`.
                        // You can attach modifiers to individual bindings as well.
                        Spawn((Binding::mouse_motion(), Scale::splat(0.1), Negate::all())),
                        Axial::right_stick().with((Scale::splat(2.0), Negate::x())),
                    )),
                )
            ]),
            crate::observers![Self::rotate, Self::apply_movement],
        )
    }

    fn update(
        mut query: Query<(&DeathCharController, &mut Transform, &Controls)>,
        time: Res<Time>,
        corn_query: super::corn::sensor::CornQuery,
        mut camera_query: Query<
            (&mut Projection, &mut Transform),
            (With<Camera>, Without<Controls>),
        >,
        mut gizmos: Gizmos,
        mut actual_fov_attenuation: Local<f32>,
    ) {
        for (controller, mut transform, controls) in query.iter_mut() {
            // attenuated as we approach edge of corn
            let mut velocity = controller.movement;

            // fake some spring force at edges by narrowing fov when you try to
            let mut target_fov = controller.fov_natural;

            // check if new position is in corn.
            let sample_offset = 0.3;
            let new_pos = transform.translation + velocity * time.delta_secs();
            let is_in_corn = corn_query.test(
                new_pos,
                Some(GradientTestSettings {
                    sample_offset,
                    num_samples: 8,
                }),
            );

            if let Some(gradient) = is_in_corn.gradient {
                // draw gizmo with gradient
                // TODO: fails at spikes bc of how the gradient is calculated, but this might be a nice exploitable bug
                let contains_edge =
                    gradient.length() * sample_offset > controller.gradient_threshold;
                gizmos.arrow(
                    new_pos,
                    new_pos + Vec3::new(gradient.x, 0.0, gradient.y) * 1.0,
                    match contains_edge {
                        true => Srgba::RED,
                        false => Srgba::GREEN,
                    },
                );

                // if gradient is pointed away from movement, treat like a wall, this lets us slide along edges
                if contains_edge {
                    let grad_vec3 = Vec3::new(gradient.x, 0.0, gradient.y).normalize();
                    let dot_of_norm = grad_vec3.dot(velocity.normalize_or_zero());
                    if dot_of_norm < 0.0 {
                        // get perpendicular component
                        let perp_component = velocity - grad_vec3 * dot_of_norm * velocity.length();

                        // narrow fov based on how much we are pushing against the edge
                        // calc before clamping velocity to wall!
                        target_fov -= -dot_of_norm
                            * controller.fov_narrowing
                            * (velocity.length() / controller.speed);

                        // friction against wall
                        // EAS: not super happy with how this feels. it's too fast near 90deg and too slow in the middleing angles.
                        // we really only want to strongly attenuate sideways movement when looking straight at wall, and very little attenuation otherwise
                        // when walking nearly parallel to wall, even the component might be too much attenuation, so we might want to redirect movement instead
                        // let strong_friction = perp_component * (1.0 - dot_of_norm.powi(6));
                        // let weak_friction = perp_component * (1.0 - dot_of_norm.powi(6));
                        // let perp_component = lerp(weak_friction, strong_friction, *actual_fov_attenuation);
                        // perp_component *= (1.0 - *actual_fov_attenuation);

                        // lerp towards perpendicular component as we exit corn field
                        // TODO better way to controll distance to edge allowed
                        // velocity = lerp(perp_component, velocity, is_in_corn.value);
                        if is_in_corn.value < 0.95 {
                            velocity = perp_component;
                        }
                    }
                }
            }

            transform.translation += velocity * time.delta_secs();

            // push down when moving
            let ratio = match controller.use_real_vel_for_drop {
                false => (controller.movement.length() / controller.speed).clamp(0.0, 1.0),
                true => (velocity.length() / controller.speed).clamp(0.0, 1.0),
            };

            let target = controller.height - ratio * controller.height_drop;
            let rate = if target < transform.translation.y {
                controller.height_rate.1 // down
            } else {
                controller.height_rate.0 // up
            };
            transform
                .translation
                .y
                .exp_ease_to(target, rate, time.delta_secs());

            // zoom in when pushing against edge
            for c in controls.iter() {
                if let Ok((mut projection, mut transform_c)) = camera_query.get_mut(c) {
                    transform_c.translation = transform.translation;
                    let Projection::Perspective(ref mut camera) = *projection else {
                        continue;
                    };

                    let rate = if target_fov < camera.fov {
                        controller.fov_rate.0 // zoom in
                    } else {
                        controller.fov_rate.1 // zoom out
                    };

                    camera
                        .fov
                        .exp_ease_to(target_fov.to_radians(), rate, time.delta_secs());

                    // store amount of current fov zoom to scale friction in movement above
                    *actual_fov_attenuation = ((controller.fov_natural - camera.fov)
                        / controller.fov_narrowing)
                        .clamp(0.0, 1.0);
                }
            }
        }
    }

    fn rotate(
        rotate: Trigger<Fired<Rotate>>,
        mut transforms: Query<(&mut Transform, Option<&Controls>), With<DeathCharController>>,
        window: Single<&Window>,
        mut camera_query: Query<&mut Transform, (With<Camera>, Without<DeathCharController>)>,
    ) {
        if window.cursor_options.grab_mode == bevy::window::CursorGrabMode::None {
            // this should be applied in bei setup
            return;
        }

        let (mut transform, controls) = transforms.get_mut(rotate.target()).unwrap();
        let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);

        yaw += rotate.value.x.to_radians();
        pitch += rotate.value.y.to_radians();

        // avoid overpitch
        pitch = pitch.clamp(-89.9_f32.to_radians(), 89.9_f32.to_radians());

        transform.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);

        for c in controls.iter().flat_map(|c| c.iter()) {
            if let Ok(mut transform_c) = camera_query.get_mut(c) {
                transform_c.rotation = transform.rotation;
            }
        }
    }

    fn apply_movement(
        movement: Trigger<Fired<Movement>>,
        mut transforms: Query<(&mut DeathCharController, &Transform)>,
    ) {
        let (mut controller, transform) = transforms.get_mut(movement.target()).unwrap();

        // Move to the camera direction.
        let rotation = transform.rotation;

        // Movement consists of X and -Z components, so swap Y and Z with negation.
        // We could do it with modifiers, but it would be weird for an action to return
        // a `Vec3` like this, so we doing it inside the function.
        let mut v = movement.value.extend(0.0).xzy();
        v.z = -v.z;

        // XXX annoying detail of bei: this stops being emitted when there is no input, so I never set movement to 0
        // NOTE: I actually like that there is always some slow movement. feels floaty
        // I could observe release, but it would be better to move all movement logic here, so we aren't doing cornsensors tests if we don't have to.
        let yaw = rotation.to_euler(EulerRot::YXZ).0;
        let rotation_y_only = Quat::from_rotation_y(yaw);
        controller.movement = rotation_y_only * v * controller.speed;
    }
}

fn spawn_dead(
    query: Query<Entity, With<Player>>,
    mut commands: Commands
){
    let player = query.single().unwrap_or_else(|_|commands.spawn((Player::bundle(), 
    // prevent panic in death controller
    Transform::default())).id());
    commands.entity(player).insert(Dead);
    commands.trigger(SpawnPlayerEvent::default());
}

