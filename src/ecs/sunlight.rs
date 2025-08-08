/// sun that rotates around.
/// TODO skybox and moon

use bevy::prelude::*;

pub struct SunPlugin;
impl Plugin for SunPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, (Revolve::revolve));
        app.add_systems(PostUpdate, update_transform_no_rotation.before(TransformSystem::TransformPropagate));
    }
}

#[derive(Debug, Clone, Component)]
#[require(Name = Name::new("Sun"))]
#[require(Revolve = Revolve(-0.009))]
pub struct Sun;

#[derive(Debug, Clone, Component)]
#[require(bevy::pbr::NotShadowReceiver)] // Moon should always get shadow cast (unless we want to have lunar eclipse)
#[require(Revolve = Revolve(-0.002))]
pub struct Moon;

#[derive(Debug, Clone, Component)]
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
#[derive(Component)]
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