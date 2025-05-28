use bevy::prelude::*;

pub struct CloneEntity {
    pub source: Entity,
    pub destination: Entity,
}

impl CloneEntity {
    // Copy all components from an entity to another.
    // Using an entity with no components as the destination creates a copy of the source entity.
    // Panics if:
    // - the source or destination entity do not exist
    // - the world does not have a type registry
    // Fails silently:
    // - the components are not registered in the type registry,
    fn clone_entity(self, world: &mut World) {
        let components = {
            let registry = world.get_resource::<AppTypeRegistry>().unwrap().read();

            world
                .get_entity(self.source)
                .unwrap()
                .archetype()
                .components()
                .filter_map(|component_id| {
                    world
                        .components()
                        .get_info(component_id)?
                        .type_id()
                })
                .filter_map(|type_id| {
                    Some(registry
                        .get(type_id)?
                        .data::<ReflectComponent>()?
                        .clone())
                })
                .collect::<Vec<_>>()
        };

        for component in components {
            let source = component
                .reflect(world.get_entity(self.source).unwrap())
                .unwrap()
                .to_dynamic();

            dbg!(&source);

            let registry = world.get_resource::<AppTypeRegistry>().unwrap().internal.to_owned();
            let mut destination = world.get_entity_mut(self.destination).unwrap();
            component.apply_or_insert_mapped(&mut destination, &*source, &registry.read().unwrap(), &mut (), bevy::ecs::relationship::RelationshipHookMode::Run);
        }
    }
}

// This allows the command to be used in systems
impl Command for CloneEntity {
    fn apply(self, world: &mut World) {
        self.clone_entity(world)
    }
}