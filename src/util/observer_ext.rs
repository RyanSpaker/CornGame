use bevy::{ecs::system::IntoObserverSystem, prelude::*};

pub trait ObserverParent{
    fn get_name(&self) -> Name;
}

pub trait ObserveAsAppExt{
    fn add_observer_as<E: Event, B: Bundle, M, C: Component+PartialEq+ObserverParent>(
        &mut self, 
        observer: impl IntoObserverSystem<E, B, M>, 
        parent_component: C
    ) -> &mut Self;
}
impl ObserveAsAppExt for App{
    fn add_observer_as<E: Event, B: Bundle, M, C: Component+PartialEq+ObserverParent>(
        &mut self, 
        observer: impl IntoObserverSystem<E, B, M>, 
        parent_component: C
    ) -> &mut Self {
        let mut query = self.world_mut().query::<(Entity, &C)>();
        let parent = match query.iter(self.world()).find(|(_, comp)| **comp==parent_component) {
            Some((entity, _)) => entity,
            _ => self.world_mut().spawn((parent_component.get_name(), parent_component)).id()
        };
        self.world_mut().add_observer(observer).set_parent(parent);
        self
    }
}

pub trait ObserveAsExt{
    fn observe_as<E: Event, B: Bundle, M, C: Component+PartialEq+ObserverParent>(
        &mut self,
        system: impl IntoObserverSystem<E, B, M>,
        parent_component: C
    ) -> &mut Self;
}
impl<'a> ObserveAsExt for EntityCommands<'a>{
    fn observe_as<E: Event, B: Bundle, M, C: Component+PartialEq+ObserverParent>(
        &mut self,
        system: impl IntoObserverSystem<E, B, M>,
        parent_component: C
    ) -> &mut Self {
        self.queue(move |entity: Entity, world: &mut World| {
            if world.get_entity(entity).is_err() {return;}
            let mut query = world.query::<(Entity, &C)>();
            let parent = match query.iter(&world).find(|(_, comp)| **comp==parent_component) {
                Some((entity, _)) => entity,
                _ => world.spawn((parent_component.get_name(), parent_component)).id()
            };
            let observer_entity = world.spawn(Observer::new(system).with_entity(entity)).id();
            world.entity_mut(parent).add_child(observer_entity);
        })
    }
}