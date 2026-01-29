//! this file implements npc brain, corn sanity, and death circle
//! TODO split up
//!

use std::{
    f32::consts::{PI, TAU},
    time::Duration,
};

use avian3d::prelude::*;
use bevy::{audio::Volume, prelude::*};
use itertools::Itertools;
use rand::random;

use crate::{
    Cli,
    ecs::{corn::{CornField, sensor::CornSensor, stored::image::ImageCarvedHexagonalShader}, death::{Alive, Die}},
    systems::character::{
        Player,
        controller::{WalkDisabled, input_handler},
    },
    util::math::lerp,
};

fn spawn_monster(asset_server: Res<AssetServer>, mut commands: Commands) {
    commands.spawn((
        Mesh3d(asset_server.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)))),
        MeshMaterial3d(asset_server.add(StandardMaterial {
            base_color: Srgba::BLACK.with_alpha(0.1).into(),
            ior: 2.0,
            alpha_mode: AlphaMode::Blend,
            ..Default::default()
        })),
        RigidBody::Kinematic,
        Collider::cuboid(1.0, 1.0, 1.0),
        CollidingEntities::default(),
        TrackerBrain,
        AudioPlayer::<AudioSource>(asset_server.load("sounds/evil_hum.ogg")),
        PlaybackSettings {
            mode: bevy::audio::PlaybackMode::Loop,
            spatial: true,
            volume: Volume::Linear(0.5),
            ..Default::default()
        },
        // TODO add corn rustle
        Transform::from_xyz(10.0, 0.0, 10.0),
    ));

    // MOVEME
    commands.spawn((
        Name::from("DeathCornField"),
        CornField,
        ImageCarvedHexagonalShader {
            center: Vec3::new(1000.0, 0.0, 0.0),
            half_extents: Vec2::splat(150.0),
            dist_between: 0.75,
            height_range: Vec2::new(0.9, 1.1),
            rand_offset_factor: 0.2,
            image: asset_server.load("textures/circle.jpg"),
        },
    ));
}

#[derive(Debug, Component, Reflect)]
pub struct TrackerBrain;

#[derive(Debug, Component, Reflect)]
pub struct TrackerTarget;

impl TrackerBrain {
    fn update(
        player: Query<(&Transform, &CornSensor), (With<TrackerTarget>, With<Alive>)>,
        mut npc: Query<(
            &mut TrackerBrain,
            &Transform,
            &mut LinearVelocity,
            &mut AngularVelocity,
        )>,
    ) {
        // TODO update volume, but don't change if you are in corn. That way monster can sneek up you
        // I think we want to avoid true spatial audio, to avoid any directional information.

        let Some((player, corn_sensor)) = player.iter().nth(0) else {
            return;
        };
        let speed = lerp(0.8, 1.2, corn_sensor.value);

        let aspeed = 0.1; //+ corn_sensor.is_in_corn / 2.0;

        for (_, t, mut vel, mut angvel) in npc.iter_mut() {
            let dist = player.translation.distance(t.translation);
            let dist = (dist - 10.0).max(0.0);
            let speed = speed
                * (lerp((dist + 1.0).log10(), dist, corn_sensor.value).clamp(0.0, 30.0) + 1.0);

            let d = player.translation - t.translation;
            if d.length() > 0.1 {
                vel.0 = d.normalize() * speed;
            } else {
                vel.0 = Vec3::ZERO;
            }
            if t.translation.y >= 2.0 {
                vel.y = -1.0;
            }

            let target_rot = t.looking_at(player.translation, Vec3::Y).rotation;
            let angle = target_rot.angle_between(t.rotation);
            if angle.abs() > 0.01 {
                angvel.0 = (t.rotation - target_rot).normalize().to_scaled_axis() * aspeed;
            } else {
                angvel.0 = Vec3::ZERO;
            }

            //if vel.linvel.magnitude()
        }
    }

    fn munch(
        mut commands: Commands,
        mut player: Query<(Entity, &RigidBodyColliders), (With<TrackerTarget>, With<Alive>)>,
        npc: Query<(Entity, &TrackerBrain, &CollidingEntities)>,
    ) {
        for (npc, _, collisions) in npc.iter() {
            for collision in collisions.iter() {
                for (id, colliders) in player.iter_mut() {
                    if colliders.iter().contains(collision) {
                        info!(npc=%npc, player=%id);
                        // commands.entity(id).insert((Dead::default(), WalkDisabled));
                        commands.queue(Die::new(id));
                    }
                }
            }
        }
    }
}

#[derive(Component)]
pub struct SpookyAudio;

#[derive(Debug, Default, Reflect, Component)]
pub struct Dead {
    timer: Timer,
}
impl Dead {
    pub fn on_death(
        mut player: Query<(Entity, &mut Dead, &Transform), (With<Player>, Added<Dead>)>,
        asset_server: Res<AssetServer>,
        mut commands: Commands,
    ) {
        for (entity, mut death, t) in player.iter_mut() {
            let mut t = *t; // XXX I have to insert pos and rot bc lightyear breaks transform->position sync

            t.translation = Vec3::new(935.0, 1.0, 0.0);
            t = t.looking_to(Vec3::X, Vec3::Y);
            commands
                .entity(entity)
                .insert((Position(t.translation), Rotation(t.rotation), t));

            commands.spawn((
                AudioPlayer::new(asset_server.load("sounds/spooky.ogg")),
                PlaybackSettings {
                    mode: bevy::audio::PlaybackMode::Despawn,
                    volume: Volume::Linear(0.1),
                    paused: false,
                    ..Default::default()
                },
                SpookyAudio,
            ));

            death.timer = Timer::from_seconds(10.0, TimerMode::Once);
        }
    }

    pub fn update(
        mut commands: Commands,
        time: Res<Time>,
        mut player: Query<(Entity, &mut Dead, &Transform), With<Player>>,
        sfx: Query<Entity, With<SpookyAudio>>,
        monster: Query<&Transform, (With<TrackerBrain>, Without<Player>)>,
    ) {
        for (id, mut death, t) in player.iter_mut() {
            let mut t = *t; // XXX I have to insert pos and rot bc lightyear breaks transform->position sync
            t.translation.x += time.delta_secs();
            death.timer.tick(time.delta());

            if death.timer.finished() {
                t.translation = Vec3::X * 50.0;
                t.rotate_around(Vec3::ZERO, Quat::from_rotation_y(TAU * random::<f32>()));
                t.translation.y = 1.5;
                if let Some(monster) = monster.iter().next() {
                    // start looking at monster in case it is close by
                    t.look_at(monster.translation, Vec3::Y);
                }
                if let Ok(sfx) = sfx.single() {
                    // sfx.stop() //XXX need sound fx system
                    commands.entity(sfx).despawn(); // TODO fade
                }
                dbg!();
                commands
                    .entity(id)
                    .remove::<Dead>()
                    .remove::<WalkDisabled>();
                commands.run_system_cached(
                    |camera: Single<Entity, With<crate::ecs::cameras::MainCamera>>,
                     mut commands: Commands| {
                        commands.entity(*camera).insert((
                            bevy_dog::settings::DoGSettings::OUTLINE_DITHER,
                            bevy_dog::settings::PassesSettings::default(),
                        ));
                    },
                );
            }
            commands
                .entity(id)
                .insert((Position(t.translation), Rotation(t.rotation), t)); // every frame cringe
        }
    }
}

pub struct NpcPlugin;
impl Plugin for NpcPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Dead>();
        app.add_systems(
            Update,
            (
                TrackerBrain::update,
                (TrackerBrain::munch, Dead::on_death, Dead::update)
                    .chain()
                    .after(input_handler),
            ),
        );

        app.add_systems(Startup, Cli::runnable(spawn_monster));
    }
}
