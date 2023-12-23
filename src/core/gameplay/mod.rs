pub mod framerate;

use bevy::prelude::*;
//use bevy::{core::FrameCount, render::view::NoFrustumCulling};
//use rand::{thread_rng, Rng};
//use crate::ecs::corn_field::corn_fields::simple_corn_field::{SimpleHexagonalCornField, SimpleRectangularCornField};
use crate::flycam::{cam_look_plugin::CamLookPlugin, cam_move_plugin::CamMovePlugin};

#[derive(Resource, Default)]
pub struct GamePlayExitState<T>(T) where T: States + Copy;

pub struct CornGamePlayPlugin<T> where T: States + Copy{
    active_state: T,
    exit_state: T
}
impl<T> CornGamePlayPlugin<T> where T: States + Copy{
    pub fn new(active_state: T, exit_state: T) -> Self {
        Self {active_state, exit_state}
    }
}
impl<T> Plugin for CornGamePlayPlugin<T> where T: States + Copy{
    fn build(&self, app: &mut App) {
        app
            .insert_resource(GamePlayExitState(self.exit_state))
            .init_resource::<CornDespawn>()
            .add_plugins((
                CamLookPlugin::<T>::new(self.active_state),
                CamMovePlugin::<T>::new(self.active_state),
                framerate::PrintFPSPlugin
            ))
            .add_systems(Update, (
                exit_state_on_key::<T>,
                spawn_corn
            ).run_if(in_state(self.active_state)));
        let corn_mat = app.world.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial::default());
        app.insert_resource(CornMaterials(corn_mat));
    }
}

fn exit_state_on_key<T: States + Copy>(
    input: Res<Input<KeyCode>>,
    exit_state: Res<GamePlayExitState::<T>>,
    mut next_state: ResMut<NextState<T>>
){
    if input.just_released(KeyCode::Escape){
        next_state.set(exit_state.0);
    }
}

#[derive(Resource, Default)]
pub struct CornDespawn(Vec<Entity>);

#[derive(Resource, Default)]
pub struct CornMaterials(Handle<StandardMaterial>);
impl From<&Handle<StandardMaterial>> for CornMaterials{
    fn from(value: &Handle<StandardMaterial>) -> Self {
        Self(value.to_owned())
    }
}

fn spawn_corn(
    //mut commands: Commands, frames: Res<FrameCount>, 
    //mut despawn_corn: ResMut<CornDespawn>,
    //material: Res<CornMaterials>
){
    /*if frames.0 == 100{
        println!("A");
        let id = commands.spawn((
            SpatialBundle::INHERITED_IDENTITY,
            SimpleRectangularCornField::new(
                Vec3::ZERO,
                Vec2::ZERO,
                UVec2::ONE,
                Vec2::new(0.5, 0.5),
                0.0
            ),
            NoFrustumCulling
        )).id();
        despawn_corn.0.push(id);
    } else if frames.0 == 101{
        println!("B");
        let id = commands.spawn((
            SpatialBundle::INHERITED_IDENTITY,
            SimpleRectangularCornField::new(
                Vec3::ZERO,
                Vec2::ZERO,
                UVec2::ONE,
                Vec2::new(1.0, 1.0),
                0.0
            ),
            NoFrustumCulling
        )).id();
        despawn_corn.0.push(id);
    } else if frames.0 == 102{
        println!("C");
        commands.entity(despawn_corn.0.remove(0)).despawn();
        let id = commands.spawn((
            SpatialBundle::INHERITED_IDENTITY,
            SimpleRectangularCornField::new(
                Vec3::ZERO,
                Vec2::ZERO,
                UVec2::new(1, 2),
                Vec2::new(0.5, 0.5),
                0.0
            ),
            NoFrustumCulling
        )).id();
        despawn_corn.0.push(id);
    } else if frames.0 == 103{
        println!("D");
        commands.entity(despawn_corn.0.remove(0)).despawn();
        let id = commands.spawn((
            SpatialBundle::INHERITED_IDENTITY,
            SimpleRectangularCornField::new(
                Vec3::ZERO,
                Vec2::ZERO,
                UVec2::new(1, 2),
                Vec2::new(1.0, 1.0),
                0.0
            ),
            NoFrustumCulling
        )).id();
        despawn_corn.0.push(id);
    } else if frames.0 == 104{
        println!("E");
        commands.entity(despawn_corn.0.remove(0)).despawn();
        let id = commands.spawn((
            SpatialBundle::INHERITED_IDENTITY,
            SimpleRectangularCornField::new(
                Vec3::ZERO,
                Vec2::ZERO,
                UVec2::new(1, 3),
                Vec2::new(0.5, 0.5),
                0.0
            ),
            NoFrustumCulling
        )).id();
        despawn_corn.0.push(id);
    }*/
    /*
    if frames.0%1u32 == 0u32{
        let mut rng = thread_rng();
        let rand: f32 = rng.gen_range(0.0..100.0);
        if (rand < 30.0 || despawn_corn.0.len()>100) && despawn_corn.0.len() > 0{
            let rand_corn = rng.gen_range(0..despawn_corn.0.len()) as usize;
            commands.entity(despawn_corn.0[rand_corn]).despawn();
            despawn_corn.0.remove(rand_corn);
        }else{
            let field_type = rng.gen_range(0.0..1.0);
            if field_type > 0.5{
                let rand_resolution: UVec2 = UVec2::new(rng.gen_range(1..50), rng.gen_range(1..50));
                despawn_corn.0.push(commands.spawn((
                    SpatialBundle::INHERITED_IDENTITY,
                    SimpleRectangularCornField::new(
                        Vec3::ZERO, 
                        Vec2::ONE, 
                        rand_resolution,
                        Vec2::new(0.8, 1.2),
                        0.0
                    ),
                    NoFrustumCulling
                )).id());
            }else{
                let rand_dist_between: f32 = rng.gen_range(1.0..50.0);
                despawn_corn.0.push(commands.spawn((
                    SpatialBundle::INHERITED_IDENTITY,
                    SimpleHexagonalCornField::new(
                        Vec3::ZERO, 
                        Vec2::ONE*50.0, 
                        rand_dist_between,
                        Vec2::new(0.8, 1.2),
                        0.0
                    ),
                    NoFrustumCulling
                )).id());
            }
        }
    }*/
}