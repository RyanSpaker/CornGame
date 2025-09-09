use bevy::{color::palettes::basic::*, ecs::{component::HookContext, world::DeferredWorld}, math::primitives, prelude::*};

use crate::systems::scenes::util::StaticComponent;

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)] #[component(on_add = SimpleMesh::on_add)]
pub enum SimpleMesh{
    Cube, Sphere, Plane
}
impl SimpleMesh{
    fn on_add(mut world: DeferredWorld, HookContext{entity, ..}: HookContext){
        world.commands().entity(entity).queue(|mut world: EntityWorldMut| {
            let Some(comp) = world.take::<Self>() else {return;};
            let res = world.resource::<SimpleMeshes>();
            world.insert(Mesh3d(match comp{
                Self::Cube => res.cube.clone(),
                Self::Sphere => res.sphere.clone(),
                Self::Plane => res.plane.clone()
            }));
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Reflect, Resource)]
pub struct SimpleMeshes{
    pub cube: Handle<Mesh>,
    pub sphere: Handle<Mesh>,
    pub plane: Handle<Mesh>,
}
impl FromWorld for SimpleMeshes{
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let mut simple_meshes = Self{cube: Handle::default(), sphere: Handle::default(), plane: Handle::default()};
        simple_meshes.cube = meshes.add(primitives::Cuboid::default());
        simple_meshes.sphere = meshes.add(primitives::Sphere::default());
        simple_meshes.plane = meshes.add(primitives::Plane3d::new(Vec3::Y, Vec2::ONE*0.5));
        simple_meshes
    }
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)] #[component(on_add = SimpleMaterial::on_add)]
pub enum SimpleMaterial{
    White, Black, Red, Green
}
impl SimpleMaterial{
    fn on_add(mut world: DeferredWorld, HookContext{entity, ..}: HookContext){
        world.commands().entity(entity).queue(|mut world: EntityWorldMut| {
            let Some(comp) = world.take::<Self>() else {return;};
            let res = world.resource::<SimpleMaterials>();
            world.insert(MeshMaterial3d(match comp{
                Self::White => res.white.clone(),
                Self::Black => res.black.clone(),
                Self::Red => res.red.clone(),
                Self::Green => res.green.clone()
            }));
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Reflect, Resource)]
pub struct SimpleMaterials{
    pub white: Handle<StandardMaterial>,
    pub black: Handle<StandardMaterial>,
    pub red: Handle<StandardMaterial>,
    pub green: Handle<StandardMaterial>
}
impl FromWorld for SimpleMaterials{
    fn from_world(world: &mut World) -> Self {
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        Self{
            white: materials.add(StandardMaterial::from_color(WHITE)),
            black: materials.add(StandardMaterial::from_color(BLACK)),
            red: materials.add(StandardMaterial::from_color(RED)),
            green: materials.add(StandardMaterial::from_color(GREEN)),
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct DefaultResourcesPlugin;
impl Plugin for DefaultResourcesPlugin{
    fn build(&self, app: &mut App) {
        app
            .init_resource::<SimpleMeshes>()
            .init_resource::<SimpleMaterials>()
            .register_type::<SimpleMesh>()
            .register_type::<SimpleMaterial>();
    }
}