use bevy::{prelude::*, core::FrameCount, render::view::NoFrustumCulling};
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
pub struct CornDespawn(Option<Entity>);

fn spawn_corn(mut commands: Commands, frames: Res<FrameCount>, mut despawn_corn: ResMut<CornDespawn>){
    if frames.0 == 100{
        despawn_corn.0 = Some(commands.spawn((
            SpatialBundle::INHERITED_IDENTITY,
            CornField::new(
                Vec3::ZERO, 
                Vec2::ONE*3.0, 
                (3, 3),
                Vec2::new(0.8, 1.2)
            ),
            NoFrustumCulling
        )).id());
    }else if frames.0 == 200{
        commands.entity(despawn_corn.0.unwrap()).despawn();
    }else if frames.0 == 300{
        commands.spawn((
            SpatialBundle::INHERITED_IDENTITY,
            CornField::new(
                Vec3::ZERO, 
                Vec2::ONE*3.0, 
                (2, 2),
                Vec2::new(0.8, 1.2)
            ),
            NoFrustumCulling
        ));
    }
}