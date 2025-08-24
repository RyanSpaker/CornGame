pub mod shader;
pub mod buffer;
pub mod asset;
pub mod cutoffs;
pub mod render;
pub mod stored;
pub mod sensor;

use bevy::{prelude::*, render::{
    batching::NoAutomaticBatching, extract_component::{ExtractComponent, ExtractComponentPlugin}, view::NoFrustumCulling
}};
use crate::{ecs::corn::{render::{ExtendWithCornMaterial, StdCornMaterial}, stored::{readback::ReadbackInit, simple::SimpleHexagonalSettings}}, scenes::lobby::LobbyScene, systems::{scenes::OnSpawnScene, util::default_resources::SimpleMaterials}, util::observer_ext::ObserverParent};

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
        app.add_systems(OnSpawnScene(LobbyScene), test_field);
    }
}

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
