pub mod shader;
pub mod buffer;
pub mod asset;
pub mod cutoffs;
pub mod render;
pub mod stored;

use bevy::{prelude::*, render::{
    batching::NoAutomaticBatching, extract_component::{ExtractComponent, ExtractComponentPlugin}, view::NoFrustumCulling
}};
use cutoffs::LodCutoffs;
use render::{ExtendWithCornMaterial, StdCornMaterial};
use stored::simple::SimpleHexagonalInitShader;
use crate::{scenes::lobby::LobbyScene, systems::{scenes::OnSpawnScene, util::default_resources::SimpleMaterials}, util::observer_ext::ObserverParent};

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
            stored::CornInitializationPlugin
        ));

        // app.add_systems(OnSpawnScene(LobbyScene), test_init);
    }
}



#[derive(Clone, Component, Default, Debug, Reflect)]
pub struct CornSensor{
    pub is_in_corn: f32
}

pub fn test_init(
    mut commands: Commands,
    default_resources: Res<SimpleMaterials>,
    std_mats: Res<Assets<StandardMaterial>>,
    mut corn_mats: ResMut<Assets<StdCornMaterial>>
){
    let mat = std_mats.get(default_resources.green.id()).unwrap().clone().extend_with_corn();
    commands.spawn((
        Name::from("Test Corn Field"),
        CornField,
        SimpleHexagonalInitShader::new(
            Vec3::ZERO, 
            Vec2::ONE*10.0, 
            1.0, 
            Vec2::new(0.9, 1.1), 
            0.0
        ),
        Transform::from_xyz(0.0, 0.0, 0.0),
        MeshMaterial3d(corn_mats.add(mat)),
        LodCutoffs(vec![20.0, 50.0, 100.0, 200.0, 500.0, 1000.0])
    ));
}
