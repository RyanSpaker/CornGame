//! CODEX (GPT-6): local walking camera; no network, input-mapping or character-controller crate.
//! CLAUDE (claude-opus-5-5): walls by avian colliders (move and slide of a capsule) instead of a
//! rectangle, so the door decides whether you can leave; the eye follows the ground below.
use std::time::Duration;

use avian3d::prelude::*;
use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};
use crate::computer::ComputerSession;

// CODEX: exercise the real controller against curved walls and furniture without a renderer.
#[cfg(test)]
#[path = "controller_tests.rs"]
mod tests;

// CODEX: the input tuning is data in lobby.corn.
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component, Default)]
pub struct Player {
    pub speed: f32,
    pub run_speed: f32,
    pub sensitivity: f32,
    pub yaw: f32,
    pub pitch: f32,
    /// CLAUDE: the body: a capsule this wide, from `step` above the feet to just over the eye
    pub radius: f32,
    /// CLAUDE: eye above the ground
    pub eye_height: f32,
    /// CLAUDE: ledges up to this high are stepped onto
    pub step: f32,
    /// CLAUDE: falling speed, m/s
    pub fall: f32,
    /// CLAUDE: set the camera's rotation from yaw/pitch when it spawns (see `face_spawn_view`); off in the
    /// cornfile (`face_spawn_view: false`) shows the first frame as the scene spawned it
    pub face_spawn_view: bool,
}

// CODEX: conservative defaults, overridden by the cornfile for this cabin.
impl Default for Player {
    // CODEX: defaults make this component usable by reflected scene construction.
    fn default() -> Self {
        Self {
            speed: 2.5,
            run_speed: 4.0,
            sensitivity: 0.002,
            yaw: -std::f32::consts::FRAC_PI_2,
            pitch: 0.0,
            radius: 0.25,
            eye_height: 1.68,
            step: 0.35,
            fall: 0.0,
            face_spawn_view: true,
        }
    }
}

// CLAUDE
const GRAVITY: f32 = 9.81;
/// CLAUDE: the capsule's top, above the eye
const HEAD: f32 = 0.08;
// CODEX: slopes up to 45 degrees are walkable; steeper contacts act as walls.
const MIN_WALKABLE_NORMAL_Y: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// CLAUDE: `on`: the real mouse is ignored, nothing grabs the cursor (scripts driving the game, dev.rs)
#[derive(Resource, Reflect, Default)]
#[reflect(Resource, Default)]
pub struct NoMouse {
    pub on: bool,
}

/// CLAUDE: the player's body, as in avian's move_and_slide example: a kinematic capsule moved by
/// `MoveAndSlide` (CustomPositionIntegration). Its own entity, so it doesn't tilt with the camera; the
/// camera (the [`Player`]) sits at its eye.
#[derive(Component)]
pub struct PlayerBody {
    pub camera: Entity,
    /// from the eye to the capsule's centre
    pub offset: Vec3,
}

// CODEX: the computer temporarily owns the camera and input while active.
pub fn plugin(app: &mut App) {
    app.add_plugins(crate::kick::plugin)
        .register_type::<Player>()
        .add_systems(Update, (spawn_body, walk).chain().after(crate::computer::update_session))
        .add_systems(PostUpdate, face_spawn_view.before(TransformSystems::Propagate));
}

// CLAUDE
// TODO hack manually matching spawned character controller view before controller active.
// The scene spawns between Update and PostUpdate, so `walk` (the only thing turning yaw/pitch into a
// rotation) first runs a frame after the camera's first render, and on web not until pointer lock.
// TODO find out why the lighting on that first rendered frame differs completely from run to run
// (e.g. extra specular highlights). Unverified guesses: textures/materials not yet loaded or bound
// (loading order), or lights/ambient/shadows not yet extracted or built while pipelines still compile.
fn face_spawn_view(mut players: Query<(&Player, &mut Transform), Added<Player>>) {
    for (player, mut transform) in players.iter_mut().filter(|(p, _)| p.face_spawn_view) {
        transform.rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);
    }
}

// CLAUDE
fn spawn_body(players: Query<(Entity, &Player, &Transform), Added<Player>>, mut commands: Commands) {
    for (camera, player, transform) in &players {
        let (bottom, top) = (player.step - player.eye_height, HEAD);
        let offset = Vec3::Y * (bottom + top) / 2.0;
        commands.spawn((
            Name::new("Player body"),
            PlayerBody { camera, offset },
            RigidBody::Kinematic,
            CustomPositionIntegration,
            // CODEX: sweeps own movement; the solver must not push props even in Ignore mode.
            Sensor,
            Collider::capsule(player.radius, (top - bottom - 2.0 * player.radius).max(0.0)),
            Transform::from_translation(transform.translation + offset),
        ));
    }
}

// CODEX: mouse capture, look and floor motion are sufficient for inspecting the lobby.
pub fn walk(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    session: Res<ComputerSession>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut players: Query<(&mut Player, &mut Transform), Without<PlayerBody>>,
    mut bodies: Query<(Entity, &PlayerBody, &Collider, &mut Transform)>,
    move_and_slide: MoveAndSlide,
    names: Query<&Name>,
    no_mouse: Res<NoMouse>,
    post_it: Res<crate::post_it::PostItSession>,
    radio_tuning: Option<Res<crate::radio::RadioTuning>>,
    mut loose: crate::kick::LooseItems,
) {
    if !crate::browser::captured() || session.active.is_some() || session.returning || post_it.blocks_input() {
        return;
    }
    if !cfg!(target_arch = "wasm32") && !no_mouse.on && buttons.just_pressed(MouseButton::Left) {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    if !cfg!(target_arch = "wasm32") && keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
    // CODEX: T temporarily owns mouse motion and freezes walking while adjusting the dial.
    if radio_tuning.is_some_and(|t| t.entity.is_some()) { return; }
    let dt = time.delta_secs().min(0.05);
    for (entity, body, collider, mut body_transform) in &mut bodies {
        let Ok((mut player, mut transform)) = players.get_mut(body.camera) else {
            continue;
        };
        // CODEX: web mouse capture belongs to the start menu and actual DOM pointer lock.
        if (cfg!(target_arch = "wasm32") || cursor.grab_mode != CursorGrabMode::None) && !no_mouse.on {
            player.yaw -= motion.delta.x * player.sensitivity;
            player.pitch = (player.pitch - motion.delta.y * player.sensitivity).clamp(-1.5, 1.5);
        }
        transform.rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);
        let input = Vec2::new(
            (keys.pressed(KeyCode::KeyD) as i32 - keys.pressed(KeyCode::KeyA) as i32) as f32,
            (keys.pressed(KeyCode::KeyS) as i32 - keys.pressed(KeyCode::KeyW) as i32) as f32,
        )
        .normalize_or_zero();
        let speed = if keys.pressed(KeyCode::ShiftLeft) {
            player.run_speed
        } else {
            player.speed
        };
        let velocity = Quat::from_rotation_y(player.yaw) * Vec3::new(input.x, 0.0, input.y) * speed;

        // CLAUDE: something else moved the camera (a teleport): the body follows it
        if body_transform.translation.distance(transform.translation + body.offset) > 1e-3 {
            body_transform.translation = transform.translation + body.offset;
        }
        // CODEX: loose props cannot block the capsule or become ground under the player's feet.
        let filter = loose.filter(entity, false);
        let moved = move_and_slide.move_and_slide(
            collider,
            body_transform.translation,
            Quat::IDENTITY,
            velocity,
            Duration::from_secs_f32(dt),
            &MoveAndSlideConfig::default(),
            &filter,
            |hit| {
                debug!(entity = ?names.get(hit.entity).ok(), point = %hit.point, normal = %*hit.normal, "player hit");
                // CODEX: Avian accepts edited hit normals: slide sideways along steep walls, not up them.
                if hit.normal.y < MIN_WALKABLE_NORMAL_Y {
                    if let Ok(wall) = Dir3::new(Vec3::new(hit.normal.x, 0.0, hit.normal.z)) {
                        *hit.normal = wall;
                    }
                }
                MoveAndSlideHitResponse::Accept
            },
        );
        let mut eye = moved.position - body.offset;
        // CODEX: wall depenetration must not lift the camera; the ground ray owns height, including ramps.
        eye.y = transform.translation.y;

        // CODEX: only search from step height above the feet, so furniture cannot become ground.
        let ground_origin = Vec3::new(eye.x, transform.translation.y - player.eye_height + player.step, eye.z);
        let ground = move_and_slide
            .spatial_query
            .cast_ray(ground_origin, Dir3::NEG_Y, player.step + 50.0, false, &filter)
            // CODEX: rounded wall faces are not ground, even when the ray catches one below step height.
            .filter(|hit| hit.normal.y >= MIN_WALKABLE_NORMAL_Y)
            .map(|hit| ground_origin.y - hit.distance);
        match ground {
            Some(ground) if eye.y - player.eye_height <= ground + player.step => {
                let target = ground + player.eye_height;
                if eye.y > target {
                    player.fall += GRAVITY * dt;
                    eye.y = (eye.y - player.fall * dt).max(target);
                } else {
                    eye.y = target;
                }
                if eye.y <= target {
                    player.fall = 0.0;
                }
            }
            Some(ground) => {
                player.fall += GRAVITY * dt;
                eye.y = (eye.y - player.fall * dt).max(ground + player.eye_height);
            }
            // nothing below (before the colliders exist): stay at this height
            None => player.fall = 0.0,
        }
        // CODEX: a separate foot sweep reaches small keys below the raised movement capsule.
        loose.sweep(&move_and_slide.spatial_query, entity,
            transform.translation - Vec3::Y * player.eye_height,
            eye - Vec3::Y * player.eye_height, player.radius);
        transform.translation = eye;
        body_transform.translation = eye + body.offset;
    }
}
