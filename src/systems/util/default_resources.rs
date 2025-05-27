use bevy::{math::primitives, prelude::*, color::palettes::basic::*};

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

#[derive(Debug, Clone, PartialEq, Eq, Reflect, Resource)]
pub struct SimpleMaterials{
    pub white: Handle<StandardMaterial>,
    pub black: Handle<StandardMaterial>,
    pub red: Handle<StandardMaterial>
}
impl FromWorld for SimpleMaterials{
    fn from_world(world: &mut World) -> Self {
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        Self{
            white: materials.add(StandardMaterial::from_color(WHITE)),
            black: materials.add(StandardMaterial::from_color(BLACK)),
            red: materials.add(StandardMaterial::from_color(RED))
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct DefaultResourcesPlugin;
impl Plugin for DefaultResourcesPlugin{
    fn build(&self, app: &mut App) {
        app
            .init_resource::<SimpleMeshes>()
            .init_resource::<SimpleMaterials>();
    }
}