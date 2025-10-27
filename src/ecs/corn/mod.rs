pub mod shader;
pub mod buffer;
pub mod asset;
pub mod cutoffs;
pub mod render;
pub mod stored;
pub mod sensor;

use bevy::{ecs::{entity, entity_disabling::Disabled}, prelude::*, render::{
    batching::NoAutomaticBatching, extract_component::{ExtractComponent, ExtractComponentPlugin}, view::NoFrustumCulling
}, scene::scene_spawner_system};
use serde::Deserialize;
use crate::{ecs::corn::{render::{ExtendWithCornMaterial, StdCornMaterial}, stored::{image::ImageCarvedHexagonalShader, readback::ReadbackInit, simple::SimpleHexagonalSettings}}, scenes::lobby::LobbyScene, systems::{scenes::OnSpawnScene, util::default_resources::SimpleMaterials}, util::observer_ext::ObserverParent};

/// Top level Tag Component for Corn Fields. 
/// Each entity with a CornField and CornPositionInitializer Component has a corresponding Buffer of corn stalk instances in the render app.
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component, ExtractComponent)]
#[reflect(Component)]
#[require(Transform, Visibility, NoFrustumCulling, NoAutomaticBatching)]
pub struct CornField;

/// System set for systems that handle corn logic
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub struct CornFieldSystemSet;

/// Parent entity for all corn field observers
#[derive(Debug, Default, Clone, PartialEq, Eq, Reflect, Component)]
pub struct CornFieldObserver;
impl ObserverParent for CornFieldObserver{
    fn get_name(&self) -> Name {
        Name::from("Corn Field Observers")
    }
}



#[derive(Component, Reflect, Default, Debug, Deserialize)]
#[reflect(Component)]
/// test component for loading cornfields from blender
pub struct BlenderCornField;

fn init_gltf_cornfield(
    corn: Query<(Entity, &BlenderCornField, Option<&Children>,  &GlobalTransform), Without<CornField>>,
    mut children: Query<(&MeshMaterial3d<StandardMaterial>, &mut Visibility)>,
    a_materials: Res<Assets<StandardMaterial>>,
    mut commands: Commands
){
    for (id, _corn, child, transform) in corn.iter() {
        if ! child.is_some_and(|c| c.len() == 1) {
            error_once!(entity = %id, "BlenderCornField must have exactly 1 child, the mesh with the image.");
            dbg!(child);
            commands.entity(id).insert(Disabled); // TODO perhaps this should be a custom Error component?, it is confusing for the path not to show up
            return; // TODO this should be system error? (or not, bc for loop)
        }
        
        info!("initializing gltf loaded cornfield entity {}", id);

        let child = child.unwrap();
        let (h_mat, mut visible) = children.get_mut(*child.first().unwrap()).unwrap();

        // hide the reference plane
        *visible = Visibility::Hidden;

        let Some(material) = a_materials.get(h_mat) else { break };
        let h_image = material.base_color_texture.clone().unwrap();

        // NOTE: we use the transform of corn object. 
        // This means that you CANNOT apply the transform in blender
        // we fully assume the plane of the model is 1x1
        // NOTE: rotation not supported yet.
        // TODO: actually use the mesh in corn render.
        // TODO: should use global transform

        let transform = transform.compute_transform();
        let center = transform.translation + Vec3::new(0.0, 0.0, 0.0); 

        let half_extents = transform.scale.xz();
        dbg!(half_extents, center);

        commands.entity(id).insert((
            CornField,
            ImageCarvedHexagonalShader::new(
                center, half_extents, 
                0.75, Vec2::new(1.1, 1.3), 0.2, 
                h_image,
            ),
            // SimpleHexagonalSettings::new(
            //     center, half_extents, 
            //     0.75, Vec2::new(1.1, 1.3), 0.2, 
            // )
        ));
    }
}

/// Adds all corn field functionality to the app
pub struct CornFieldComponentPlugin;
impl Plugin for CornFieldComponentPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<CornField>()
            .add_plugins(ExtractComponentPlugin::<CornField>::default());

        app.add_plugins((
            shader::CornShadersPlugin, 
            buffer::CornBufferPlugin, 
            asset::CornModelPlugin, 
            cutoffs::CornCutoffPlugin,
            render::CornRenderPlugin,
            stored::CornInitializationPlugin,
        ));

        app.add_plugins(sensor::CornSensorPlugin);

        // blender defined cornfields
        app.add_systems(PostUpdate, init_gltf_cornfield.after(TransformSystem::TransformPropagate) ); // systems that post-process scenes should run after SceneSpawn, idk if this is exactly right
        app.register_type::<BlenderCornField>(); // needed for loading from gltf
        // app.add_systems(OnSpawnScene(LobbyScene), test_field);
    }
}

// TODO make test scenes commands and hook into cli and editor command prompt
pub fn test_field(
    mut commands: Commands,
    resource: Res<SimpleMaterials>,
    std_mats: Res<Assets<StandardMaterial>>,
    mut corn_mats: ResMut<Assets<StdCornMaterial>>
){
    let Some(green) = std_mats.get(resource.green.id()) else {return};
    let corn_mat = corn_mats.add(green.clone().extend_with_corn());
    commands.spawn((
        Name::from("Test Corn Field"),
        CornField,
        SimpleHexagonalSettings{
            center: Vec3::new(0.0, 0.0, 0.0),
            half_extents: Vec2::new(5.0, 5.0),
            dist_between: 1.0,
            height_range: Vec2::new(0.9, 1.1),
            rand_offset_factor: 0.1
        },
        MeshMaterial3d(corn_mat),
    ));
}
