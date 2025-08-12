/// Code to help resolve entity relations in scenes to actual entitys

use bevy::{ecs::system::SystemParam, prelude::*, scene::SceneInstance};

#[derive(Debug, Clone, Component, Reflect)]
pub enum EntityPointer{
    SameSceneName(String)
}

#[derive(Debug, Clone, SystemParam)]
pub struct EntityResolver<'w, 's> {
    children: Query<'w, 's, &'static Children>,
    parents: Query<'w, 's, &'static ChildOf>,
    scene: Query<'w, 's, Entity, With<SceneInstance>>, 
    name: Query<'w, 's, &'static Name>, // TODO perhaps require a marker component on the target to speed this up
}

impl<'w, 's> EntityResolver<'w, 's> {
    pub fn resolve(&self, start: Entity, pointer: &EntityPointer) -> Result<Entity, ResolutionError> {
        match pointer {
            EntityPointer::SameSceneName(name) => {
                let scene_root = match self.parents.iter_ancestors(start).find(|e|self.scene.contains(*e)) {
                    Some(e) => e,
                    None => return Err(ResolutionError::NoSceneAncestor),
                };

                let name = Name::new(name.clone());
                match self.children.iter_descendants(scene_root).find(|e| self.name.get(*e) == Ok(&name)){
                    Some(e) => Ok(e),
                    None => return Err(ResolutionError::NameNotFound{
                        root: Some(scene_root),
                        name: name.as_str().to_string()
                    })
                }
            }
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ResolutionError{
    #[error("entity has no scene ancestor")]
    NoSceneAncestor,
    #[error("'{name}' not found under {root:?}")]
    NameNotFound{
        root: Option<Entity>,
        name: String,
    }
}