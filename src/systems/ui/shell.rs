use std::collections::BTreeSet;

use bevy::{ecs::{component::ComponentId, entity::Entity, world::{EntityMut, EntityRef, World}}, reflect::{TypeRegistration, TypeRegistryArc}};
use bevy::prelude::*;

/// My attempt at an easy to use keyboard navigator.

#[derive(Debug, Clone)]
struct Cursor {
    entities: Vec<Entity>,
}

struct WorldCursor<'a> {
    world: &'a mut World,
    cursor: Cursor,
    next_cursor: Cursor, 
}

impl<'a> WorldCursor<'a> {
    fn iter(&mut self) -> impl Iterator<Item = EntityMut>{
        match self.world.get_entity_mut(self.cursor.entities.as_slice()){
            Ok(s) => s.into_iter(),
            Err(e) => {
                error!("{}", e);
                return Default::default()  
            },
        }
    }

    fn type_registry(&self) -> TypeRegistryArc{
        let registry = self.world.get_resource::<AppTypeRegistry>();
        registry.unwrap().0.clone()
    }

    fn type_lookup(&self, name: &str) -> Option<TypeRegistration> {
        // TODO what about dynamic components have name but not type.
        if let Some(id) = self.type_registry().read().get_with_type_path(name).cloned(){
            return Some(id);
        }
        if let Some(id) = self.type_registry().read().get_with_short_type_path(name).cloned(){
            return Some(id);
        }

        return None
    }
}

struct ComponentQuery {
    name: String,
}

impl Function for ComponentQuery {
    fn apply(&self, cursor: &mut WorldCursor) {
        let Some(id) = cursor.type_lookup(&self.name) else {
            error!("invalid component {}", self.name);
            return;
        };

        let mut next : Vec<Entity> = Vec::new(); // TODO don't do this

        for c in cursor.iter(){
            //xxx what if it's not a Component, but in registry
            if c.contains_type_id(id.type_id()){
                next.push(c.id());
            }
        }

        cursor.cursor.entities = next;
    }

    fn complete(cursor: &WorldCursor, text: &str) -> Vec<Self> {
        let mut out = Vec::new();
        for a in cursor.type_registry().read().iter(){
            let p = a.type_info().type_path();
            if p.contains(text){
                out.push(Self{name:p.to_string()});
            }
        }
        out
    }
}

struct Drill {
    name: String,
    next: Option<Box<Drill>>   
}

trait Function : Sized {
    fn apply(&self, cursor: &mut WorldCursor);

    fn complete(cursor: &WorldCursor, text: &str) -> Vec<Self>{
        Default::default()
    }
}