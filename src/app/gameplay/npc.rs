use std::{f32::consts::{PI, TAU}, time::Duration};

use bevy::{audio::Volume, prelude::*};
use bevy_rapier3d::{prelude::*};
use rand::random;

use crate::{app::audio::Spooky, ecs::corn::field::cf_image_carved::CornSensor, util::lerp};

#[derive(Debug, Component, Reflect)]
pub struct TrackerBrain;

#[derive(Debug, Component, Reflect)]
pub struct TrackerTarget;

impl TrackerBrain {
    fn update(
        player : Query<(&Transform, &CornSensor), (With<TrackerTarget>, Without<Dead>)>,
        mut npc: Query<(&mut TrackerBrain, &Transform, &mut Velocity)>,
    ){
        let Some((player, corn_sensor)) = player.iter().nth(0) else { return };
        let speed = lerp(0.8, 1.2, corn_sensor.is_in_corn);

        let aspeed = 0.1; //+ corn_sensor.is_in_corn / 2.0;

        for (_, t, mut vel) in npc.iter_mut(){
            let dist = player.translation.distance(t.translation);
            let dist = (dist - 10.0).max(0.0);
            let speed = speed * (
                lerp( 
                    (dist+1.0).log10(), 
                    dist, 
                    corn_sensor.is_in_corn
                ).clamp(0.0, 30.0) + 1.0
            );

            let d = player.translation - t.translation;
            if d.length() > 0.1{
                vel.linvel = d.normalize() * speed;
                //dbg!(vel.linvel);
            }else{
                vel.linvel = Vec3::ZERO;
            }

            let target_rot = t.looking_at(player.translation, Vec3::Y).rotation;
            let angle = target_rot.angle_between(t.rotation);
            if angle.abs() > 0.01 {
                vel.angvel = (t.rotation - target_rot).normalize().to_scaled_axis() * aspeed;
            }else{
                vel.angvel = Vec3::ZERO;
            }
            
            //if vel.linvel.magnitude()
        }
    }

    fn munch(
        mut commands: Commands,
        mut player : Query<Entity, (With<TrackerTarget>,Without<Dead>)>,
        npc: Query<(&mut TrackerBrain, &CollidingEntities)>,
    ){
        for (_, collisions) in npc.iter(){
            if collisions.is_empty() {continue}
            for id in player.iter_mut(){
                if collisions.contains(id){
                    commands.entity(id).insert(Dead::default());
                }
            }
        }
    }
}

#[derive(Debug, Default, Reflect, Component)]
pub struct Player;

#[derive(Debug, Default, Reflect, Component)]
pub struct Dead {
    timer: Timer
}
impl Dead {
    pub fn on_death(
        mut player : Query<(&mut Dead, &mut Transform), (With<Player>, Added<Dead>)>,
        sfx: Query<&AudioSink, With<Spooky>>,
    ){
        for (mut death, mut t) in player.iter_mut(){
            dbg!();

            t.translation = Vec3::new(935.0, 1.0, 0.0);
            *t = t.looking_to(Vec3::X, Vec3::Y);
            if let Ok(sfx) = sfx.get_single() {
                sfx.play() //XXX need sound fx system
            }

            death.timer = Timer::from_seconds(10.0, TimerMode::Once);
        }
    }

    pub fn update(
        mut commands : Commands,
        time: Res<Time>,
        mut player : Query<(Entity, &mut Dead, &mut Transform), With<Player>>,
        sfx: Query<&AudioSink, With<Spooky>>,
        monster: Query<&Transform, (With<TrackerBrain>,Without<Player>)>
    ){
        for (id, mut death, mut t) in player.iter_mut(){
            t.translation.x += time.delta_seconds();
            death.timer.tick(time.delta());

            if death.timer.finished() {
                t.translation = Vec3::X * 50.0;
                t.rotate_around(Vec3::ZERO, Quat::from_rotation_y( TAU * random::<f32>() ));
                t.translation.y = 1.5;
                if let Some(monster) = monster.iter().next(){
                    // start looking at monster in case it is close by
                    t.look_at(monster.translation, Vec3::Y);
                }
                if let Ok(sfx) = sfx.get_single() {
                    sfx.pause() //XXX need sound fx system
                }
                commands.entity(id).remove::<Dead>();
            }
        }
    }
}


pub struct NpcPlugin;
impl Plugin for NpcPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (TrackerBrain::update, TrackerBrain::munch, Dead::on_death, Dead::update.after(Dead::on_death)));
    }
}