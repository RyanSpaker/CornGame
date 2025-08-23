use bevy::prelude::*;

#[derive(Clone, Component, Default, Debug, Reflect)]
pub struct CornSensor{
    pub is_in_corn: f32
}

pub struct CornSensorPlugin;
impl Plugin for CornSensorPlugin{
    fn build(&self, app: &mut App) {
        app.register_type::<CornSensor>();
    }
}