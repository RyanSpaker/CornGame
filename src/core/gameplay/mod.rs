use bevy::{prelude::*, core::FrameCount, render::view::NoFrustumCulling};
use rand::{thread_rng, Rng};
use crate::{flycam::{cam_look_plugin::CamLookPlugin, cam_move_plugin::CamMovePlugin}, ecs::corn_field::CornField};

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
                CamMovePlugin::<T>::new(self.active_state)
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
pub struct CornMaterials(pub Handle<StandardMaterial>);
impl From<&Handle<StandardMaterial>> for CornMaterials{
    fn from(value: &Handle<StandardMaterial>) -> Self {
        Self(value.to_owned())
    }
}

fn spawn_corn(
    mut commands: Commands, frames: Res<FrameCount>, 
    mut despawn_corn: ResMut<CornDespawn>,
    material: Res<CornMaterials>
){
    
    /*if frames.0 == 10u32{
        commands.spawn((
            SpatialBundle::INHERITED_IDENTITY,
            CornField::new(
                Vec3::ZERO, 
                Vec2::ONE*5.0, 
                (20, 20),
                Vec2::new(0.8, 1.2)
            ),
            NoFrustumCulling
        ));
    }*/
    /*
    if frames.0%1u32 == 0u32{
        let mut rng = thread_rng();
        let rand: f32 = rng.gen_range(0.0..100.0);
        if (rand < 50.0 || despawn_corn.0.len()>31) && despawn_corn.0.len() > 0{
            if despawn_corn.0.len() > 0{
                let rand_corn = rng.gen_range(0..despawn_corn.0.len()) as usize;
                commands.entity(despawn_corn.0[rand_corn]).despawn();
                despawn_corn.0.remove(rand_corn);
            }
        }else{
            let rand_count = rng.gen_range(1..10);
            let rand_count2 = rng.gen_range(1..10);
            despawn_corn.0.push(commands.spawn((
                SpatialBundle::INHERITED_IDENTITY,
                CornField::new(
                    Vec3::new(rng.gen_range(-50.0..50.0), 0.0, rng.gen_range(-50.0..50.0)), 
                    Vec2::ONE*Vec2::new(rand_count as f32, rand_count2 as f32), 
                    (rand_count, rand_count2),
                    Vec2::new(0.8, 1.2)
                ),
                material.0.to_owned(),
                NoFrustumCulling
            )).id());
        }
    }*/
}