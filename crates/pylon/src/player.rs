//! CLAUDE (claude-opus-5-5): the walking camera. Mouse look (click to grab the cursor, Escape to let go),
//! WASD, Shift to run, at a fixed eye height: every walkable surface in the pylon is at y = 0. It can't
//! walk into a [`Blocker`].

use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use crate::range::RangeTarget;

pub fn plugin(app: &mut App) {
    app.register_type::<Player>().register_type::<Blocker>().add_systems(Update, (grab, (look, walk).chain()).chain());
}

#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component, Default)]
#[require(RangeTarget, Transform)]
pub struct Player {
    /// metres per second
    pub walk: f32,
    pub run: f32,
    /// collision radius
    pub radius: f32,
    /// degrees per 100 pixels of mouse motion
    pub look: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self { walk: 3.7, run: 6.2, radius: 0.22, look: 11.5, yaw: 0.0, pitch: 0.0 }
    }
}

/// a box (in this entity's space) the player can't enter. only its footprint on XZ matters, and only
/// where it reaches between the floor and the player's eyes
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
#[require(Transform)]
pub struct Blocker {
    pub half_size: Vec3,
}

fn grab(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    if keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

fn look(motion: Res<AccumulatedMouseMotion>, cursor: Single<&CursorOptions, With<PrimaryWindow>>, mut players: Query<(&mut Player, &mut Transform)>) {
    for (mut player, mut transform) in &mut players {
        if cursor.grab_mode != CursorGrabMode::None && motion.delta != Vec2::ZERO {
            let per_pixel = (player.look / 100.0).to_radians();
            player.yaw -= motion.delta.x * per_pixel;
            player.pitch = (player.pitch - motion.delta.y * per_pixel).clamp(-1.5, 1.5);
        }
        let rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);
        if transform.rotation != rotation {
            transform.rotation = rotation;
        }
    }
}

fn walk(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut players: Query<(&Player, &mut Transform)>,
    blockers: Query<(&Blocker, &GlobalTransform)>,
) {
    for (player, mut transform) in &mut players {
        let mut input = Vec2::ZERO;
        for (key, dir) in [(KeyCode::KeyW, Vec2::NEG_Y), (KeyCode::KeyS, Vec2::Y), (KeyCode::KeyA, Vec2::NEG_X), (KeyCode::KeyD, Vec2::X)] {
            if keys.pressed(key) {
                input += dir;
            }
        }
        if input == Vec2::ZERO {
            continue;
        }
        let speed = if keys.pressed(KeyCode::ShiftLeft) { player.run } else { player.walk };
        let step = Vec2::from_angle(-player.yaw).rotate(input.normalize()) * speed * time.delta_secs().min(0.05);
        let mut at = transform.translation + Vec3::new(step.x, 0.0, step.y);
        // twice, for corners where two blockers meet
        for _ in 0..2 {
            for (blocker, global) in &blockers {
                at = push_out(at, player.radius, blocker, global);
            }
        }
        transform.translation = at;
    }
}

/// `eye` moved out of the blocker's footprint (grown by `radius`), if it's inside
fn push_out(eye: Vec3, radius: f32, blocker: &Blocker, global: &GlobalTransform) -> Vec3 {
    let to_local = global.affine().inverse();
    let local = to_local.transform_point3(eye);
    let half = blocker.half_size;
    // the player spans the floor (eye height below the eye) to the eye
    let feet = to_local.transform_point3(Vec3::new(eye.x, 0.0, eye.z)).y;
    if local.y.min(feet) > half.y || local.y.max(feet) < -half.y {
        return eye;
    }
    let p = local.xz();
    let closest = p.clamp(-half.xz(), half.xz());
    let out = p - closest;
    let moved = if out != Vec2::ZERO {
        if out.length() >= radius {
            return eye;
        }
        closest + out.normalize() * radius
    } else {
        // inside: leave by the nearest side
        let gap = half.xz() - p.abs();
        if gap.x < gap.y {
            Vec2::new(p.x.signum() * (half.x + radius), p.y)
        } else {
            Vec2::new(p.x, p.y.signum() * (half.z + radius))
        }
    };
    global.transform_point(Vec3::new(moved.x, local.y, moved.y)).with_y(eye.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blockers_push_out() {
        let wall = Blocker { half_size: Vec3::new(1.0, 2.0, 0.1) };
        let at = GlobalTransform::from(Transform::from_xyz(0.0, 2.0, 0.0));
        // walking into the face
        let eye = push_out(Vec3::new(0.0, 1.7, 0.2), 0.22, &wall, &at);
        assert!((eye.z - 0.32).abs() < 1e-5, "{eye}");
        // far enough
        assert_eq!(push_out(Vec3::new(0.0, 1.7, 0.5), 0.22, &wall, &at), Vec3::new(0.0, 1.7, 0.5));
        // a box overhead doesn't block
        let high = GlobalTransform::from(Transform::from_xyz(0.0, 5.0, 0.0));
        assert_eq!(push_out(Vec3::new(0.0, 1.7, 0.0), 0.22, &wall, &high), Vec3::new(0.0, 1.7, 0.0));
        // turned: the wall now runs along Z
        let turned = GlobalTransform::from(Transform::from_xyz(0.0, 2.0, 0.0).with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)));
        let eye = push_out(Vec3::new(0.15, 1.7, 0.5), 0.22, &wall, &turned);
        assert!((eye.x - 0.32).abs() < 1e-4 && (eye.z - 0.5).abs() < 1e-4, "{eye}");
    }
}
