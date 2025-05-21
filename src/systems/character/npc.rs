use bevy::prelude::*;


#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component)]
pub struct NpcBrain;

#[derive(Debug, Clone, Default, Component, Reflect)]
#[reflect(Component)]
pub struct NpcController;

impl NpcController {
    fn system(query: Query<&NpcController>){
        for npc in query {
            
        }
    }
}