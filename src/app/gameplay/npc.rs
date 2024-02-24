use bevy::{audio::Volume, prelude::*};
use bevy_rapier3d::{prelude::*};

use crate::app::audio::Spooky;

#[derive(Debug, Component, Reflect)]
pub struct TrackerBrain;

#[derive(Debug, Component, Reflect)]
pub struct TrackerTarget;

impl TrackerBrain {
    fn update(
        player : Query<&Transform, With<TrackerTarget>>,
        mut npc: Query<(&mut TrackerBrain, &Transform, &mut Velocity)>,
    ){
        let Some(player) = player.iter().nth(0) else { return };
        let speed = 1.0;
        let aspeed = 0.1;

        for (_, t, mut vel) in npc.iter_mut(){
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
        asset_server: Res<AssetServer>,
        mut player : Query<(Entity, &mut Transform), With<TrackerTarget>>,
        npc: Query<(&mut TrackerBrain, &CollidingEntities)>,
        sfx: Query<&mut AudioSink, With<Spooky>>,
    ){
        for (_, collisions) in npc.iter(){
            if collisions.is_empty() {continue}

            dbg!(collisions.iter().nth(0));
            for (id,mut t) in player.iter_mut(){
                dbg!(id);

                if collisions.contains(id){
                    dbg!();

                    t.translation = Vec3::new(935.0, 1.0, 0.0);
                    *t = t.looking_to(Vec3::X, Vec3::Y);
                    if let Ok(sfx) = sfx.get_single() {
                        sfx.play()
                    }
                }
            }
        }
    }
}

pub struct NpcPlugin;
impl Plugin for NpcPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (TrackerBrain::update, TrackerBrain::munch));
    }
}