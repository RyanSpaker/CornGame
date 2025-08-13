/// sun that rotates around.
/// TODO skybox and moon

use bevy::{ecs::{component::HookContext, world::DeferredWorld}, math::VectorSpace, prelude::*};

pub struct SunPlugin;
impl Plugin for SunPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, (Revolve::revolve));
        app.add_systems(PostUpdate, update_transform_no_rotation.before(TransformSystem::TransformPropagate));
        app.add_systems(PostUpdate, enable_shadows_on_light_added);
        app.register_type::<Sun>();
        app.register_type::<Moon>();
        app.register_type::<Revolve>();
        app.register_type::<BlenderMoon>();
        app.register_type::<BlenderShadows>();
    }
}

#[derive(Debug, Clone, Component, Reflect)]
#[require(Name = Name::new("Sun"))]
#[require(Revolve = Revolve(-0.009))]
#[reflect(Component)]
pub struct Sun;

#[derive(Debug, Clone, Default, Component, Reflect)]
#[require(bevy::pbr::NotShadowReceiver)] // Moon should always get shadow cast (unless we want to have lunar eclipse)
#[require(Revolve = Revolve(-0.002))]
#[reflect(Component)]
pub struct Moon;

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct Revolve(
    /// radians pers minute
    f32
);
impl Revolve {
    fn revolve(
        mut sun: Query<(&mut Transform, &Revolve)>,
        time: Res<Time>,
    ){
        for (mut sun, rev) in sun.iter_mut() {
            let quat = Quat::from_axis_angle(sun.local_x().into(), rev.0 * time.delta_secs());
            sun.rotate_around(Vec3::ZERO, quat);
            
            // originally I only did this at spawn, but it's nicer to not have to rotate the thing in blender.
            *sun = sun.looking_at(Vec3::ZERO, Dir3::Y);
            sun.rotation = sun.rotation.normalize();
        }
    }
}

// ??? Does not work
// #[derive(Debug, Clone, Component)]
// pub struct PointUp;
// impl PointUp {
//     fn pointup(
//         mut sun: Query<(&mut GlobalTransform), With<PointUp>>,
//     ){
//         for (mut gt) in sun.iter_mut() {
//             *gt = GlobalTransform::from(gt.compute_transform().with_rotation(Quat::IDENTITY));
//         }
//     }
// }

// Placed on the child entity, this component will cause the child to have its transform
// updated to match the parent entity's transform. But it will not inherit the parent's
// rotation.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct NoRotationChild;

/// there is no way to inherit position but not rotation from the parent entity transform yet
/// see: https://github.com/bevyengine/bevy/issues/1780
/// so labels will rotate with ships unless we fiddle with it:
/// TODO: remove this when the issue is fixed second
/// TODO: logic here is wonky, what if intermediate parents don't have transform, is transform even propogated?
pub fn update_transform_no_rotation(
    parents: Query<&ChildOf>,
    mut q_text: Query<(Entity, &mut Transform), With<NoRotationChild>>,
    q_parents: Query<&GlobalTransform, Without<NoRotationChild>>,
) {
    for (entity, mut transform) in q_text.iter_mut() {
        if let Some(parent_transform) = parents.iter_ancestors(entity)
                .filter_map(
                    |e| q_parents.get(e).ok()
                )
                .next()
        {
            // global transform propagation system will make the rotation 0 now
            transform.rotation = parent_transform.rotation().inverse().normalize();
        }
    }
}

/// position sun using a blender empty, but scale and put at correct distance
#[derive(Debug, Clone, Component, Reflect)]
#[require(Moon)]
#[component(on_add = BlenderMoon::on_add)]
#[reflect(Component)]
pub struct BlenderMoon;

impl BlenderMoon {
    // scale transform to be 1000m from origin, and update scale so it looks the same size
    fn on_add(
        mut world: DeferredWorld,
        HookContext { entity, .. }: HookContext,
    ) {
        // Get the original transform if it exists
        if let Some(mut original_transform) = world.get_mut::<Transform>(entity) {
            // Calculate the direction from the origin to the current position
            let direction = original_transform.translation.normalize_or_zero();
            // Place at 1000 units along that direction (default to Y if zero)
            let translation = direction * 1000.0;

            if translation == Vec3::ZERO {
                warn!("no zero");
                return;
            }

            // Compute apparent size scaling factor
            let scale = original_transform.scale * (translation.length() / original_transform.translation.length());
            // Preserve rotation and scale, update translation and scale for apparent size
            *original_transform = Transform {
                translation,
                rotation: original_transform.rotation,
                scale,
            };
        } else {
            warn!("no transform");
        }
    }
}

#[derive(Debug, Clone, Component, Reflect)]
#[component(on_add = BlenderShadows::on_add)]
#[reflect(Component)]
pub struct BlenderShadows;

impl BlenderShadows {
    fn on_add(
        mut world: DeferredWorld,
        HookContext { entity, .. }: HookContext,
    ) {
        if let Some(mut light) = world.get_mut::<bevy::pbr::DirectionalLight>(entity) {
            light.shadows_enabled = true;
        } else if let Some(mut light) = world.get_mut::<bevy::pbr::PointLight>(entity) {
            light.shadows_enabled = true;
        } else if let Some(mut light) = world.get_mut::<bevy::pbr::SpotLight>(entity) {
            light.shadows_enabled = true;
        } else {
            warn!("BlenderShadows: No supported light component found on entity {:?}", entity);
        }
    }
}

/// System to enable shadows if a light component is added after BlenderShadows
pub fn enable_shadows_on_light_added(
    mut dir_lights: Query<(&mut bevy::pbr::DirectionalLight, Entity), (Added<bevy::pbr::DirectionalLight>, With<BlenderShadows>)>,
    mut point_lights: Query<(&mut bevy::pbr::PointLight, Entity), (Added<bevy::pbr::PointLight>, With<BlenderShadows>)>,
    mut spot_lights: Query<(&mut bevy::pbr::SpotLight, Entity), (Added<bevy::pbr::SpotLight>, With<BlenderShadows>)>,
) {
    for (mut light, _) in dir_lights.iter_mut() {
        light.shadows_enabled = true;
    }
    for (mut light, _) in point_lights.iter_mut() {
        light.shadows_enabled = true;
    }
    for (mut light, _) in spot_lights.iter_mut() {
        light.shadows_enabled = true;
    }
}
