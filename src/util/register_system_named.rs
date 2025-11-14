/// register system and stick it's metadata into a resource so we can access it at runtime
/// bc RegisteredSystem is pub(crate) for some reason

use bevy::prelude::*;
use bevy::ecs::system::SystemId;

#[extension_trait::extension_trait]
pub impl RegisterSystemNamed for App {
    // TODO support fallible
    fn register_system_named<M>(
        &mut self,
        system: impl IntoSystem<(), (), M> + 'static,
    ) -> SystemId<(), Result>
    {
        let system = IntoSystem::into_system(system);
        
        let name = system.name().to_string();
        let system = system.pipe(|| Ok(()));
        let id = self.register_system(system);
        
        self.world_mut().init_resource::<SystemMap>();       
        let mut map = self.world_mut().resource_mut::<SystemMap>();
        map.0.push(SystemInfo{
            id, name 
        }); 

        id
    }
}

#[derive(Debug, Clone)]
pub struct SystemInfo{
    pub id: SystemId<(), Result>, 
    pub name: String
}

#[derive(Debug, Default, Clone, Resource)]
pub struct SystemMap(Vec<SystemInfo>);

impl SystemMap {
    pub fn get_system(&self, name: &str) -> Option<SystemInfo> {
        fn match_path(value: &str, full_path: &str)->bool{
            full_path == value || full_path.ends_with(value)
        }

        self.0.iter().find(|v| match_path(name, v.name.as_str())).cloned()
    }
}