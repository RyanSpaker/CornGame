//! Alternative to Children for things that shouldn't propogate Transform or Visiblity
//! You can opt into propogation by writing your own custom systems. 
//! Consider making a custom relation instead. (Inheritance would actually be nice here, but alas.)

use bevy::{ecs::relationship::Relationship, prelude::*, reflect::GetTypeRegistration};

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[relationship_target(relationship = PartOf)]
pub struct Parts(Vec<Entity>);

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[relationship(relationship_target = Parts)]
pub struct PartOf(pub Entity);

/// NOTE: perhaps this would be better as a macro?
#[derive(Debug, Clone, Component, Reflect)]
#[relationship_target(relationship = RelOf<T>)]
#[reflect(Component)]
pub struct Rel<T:Component>{
    #[relationship] 
    collection: Vec<Entity>,
    #[reflect(ignore)] 
    _phantom_data: std::marker::PhantomData<T>
}

#[derive(Debug, Clone, Component, Reflect)]
#[relationship(relationship_target = Rel<T>)]
#[reflect(Component)]
pub struct RelOf<T:Component>{
    #[relationship] 
    pub entity: Entity, 
    
    #[reflect(ignore)] 
    _phantom_data: std::marker::PhantomData<T>
}

// impl<T:Component> Relationship for RelOf<T> {
//     type RelationshipTarget = Rel<T>;

//     fn get(&self) -> Entity {
//         self.0
//     }

//     fn from(entity: Entity) -> Self {
//         Self(entity, default())
//     }
// }


impl<T: Component> RelOf<T>{
    pub fn new(entity: Entity) -> Self{
        Self {
            entity,
            _phantom_data: std::marker::PhantomData,
        }
    }
}

// impl<T:Component> RelationshipTarget for Rel<T> {
//     const LINKED_SPAWN: bool = false;

//     type Relationship = RelOf<T>;
//     type Collection = Vec<Entity>;

//     fn collection(&self) -> &Self::Collection {
//         &self.0 
//     }

//     fn collection_mut_risky(&mut self) -> &mut Self::Collection {
//         &mut self.0
//     }

//     fn from_collection_risky(collection: Self::Collection) -> Self {
//         Self(collection, default())
//     }
// }

#[derive(Default)]
pub struct RelPlugin<T>(std::marker::PhantomData<T>);
impl<T:Component + Reflect + TypePath + GetTypeRegistration> Plugin for RelPlugin<T> {
    fn build(&self, app: &mut App) {
        app.register_type::<Rel<T>>();
        app.register_type::<RelOf<T>>();
        app.register_type::<T>();
    }
}

/// TODO Nav trait. Is like iterator but can go along any axis.

trait Nav : Sized {
    type Item;

    fn next<A>(&mut self) -> Option<Self::Item> where A : Axis<Self>;
}

trait Axis<N: Nav> {

}

impl<'a, T : Relationship> Axis<WorldNav<'a>> for T {

}

struct WorldNav<'a>{
    world: &'a World,
}

impl<'a> Nav for WorldNav<'a> {
    type Item = ();

    fn next<A>(&mut self) -> Option<Self::Item> where A : Axis<Self>{
        todo!()
    }
}