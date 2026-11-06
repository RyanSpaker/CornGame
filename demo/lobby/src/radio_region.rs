//! CODEX — GPT-6: linear reception along rays between independently transformed region boundaries.
use bevy::{math::Affine3A, prelude::*};

// CODEX: Treat each unit circle marker as the outline of a unit sphere transformed into an ellipsoid.
// Full affine inverses preserve parent offsets, rotations and shear from rotated nonuniform scales.
// The inner center defines the ray; the outer marker must enclose the inner shape for a complete fade.
pub(super) fn strength(point: Vec3, inner: &GlobalTransform, outer: &GlobalTransform) -> f32 {
    let (Some(inner_inverse), Some(outer_inverse)) = (inverse(inner), inverse(outer)) else {
        return 0.0;
    };
    if !point.is_finite() {
        return 0.0;
    }
    let outer_point = outer_inverse.transform_point3(point);
    if outer_point.length_squared() >= 1.0 {
        return 0.0;
    }
    let inner_radius = inner_inverse.transform_point3(point).length();
    if inner_radius <= 1.0 {
        return 1.0;
    }

    // CODEX: Parameterize center + t * (radio - center), so the radio itself is at t = 1.
    let center = inner.translation();
    let direction = point - center;
    let inner_hit = inner_radius.recip();
    let origin = outer_inverse.transform_point3(center);
    let ray = outer_inverse.transform_vector3(direction);
    let a = ray.length_squared();
    let b = origin.dot(ray);
    let c = origin.length_squared() - 1.0;
    let discriminant = b * b - a * c;
    if a <= 0.0 || discriminant < 0.0 {
        return 0.0;
    }
    // CODEX: The positive exit root is the far intersection, beyond the radio inside the outer shape.
    let outer_hit = (-b + discriminant.sqrt()) / a;
    let interval = outer_hit - inner_hit;
    if interval <= 0.0 {
        return 0.0;
    }
    ((outer_hit - 1.0) / interval).clamp(0.0, 1.0)
}

// CODEX: Collapsed marker axes disable the region instead of generating NaNs in the audio mix.
fn inverse(transform: &GlobalTransform) -> Option<Affine3A> {
    let affine = transform.affine();
    let determinant = affine.matrix3.determinant();
    if !determinant.is_finite() || determinant == 0.0 {
        return None;
    }
    let inverse = affine.inverse();
    inverse.is_finite().then_some(inverse)
}

// CODEX: Test the spatial contract independently of audio hardware or a particular Blender scene.
#[cfg(test)]
mod tests {
    use super::*;

    // CODEX: Concentric circles use world distance linearly, including both exact boundaries.
    #[test]
    fn circular_falloff_is_linear_between_intersections() {
        let inner = GlobalTransform::from(Transform::from_scale(Vec3::splat(2.0)));
        let outer = GlobalTransform::from(Transform::from_scale(Vec3::splat(4.0)));
        for (distance, expected) in [
            (0.0, 1.0),
            (2.0, 1.0),
            (2.5, 0.75),
            (3.0, 0.5),
            (3.5, 0.25),
            (4.0, 0.0),
            (5.0, 0.0),
        ] {
            assert!((strength(Vec3::X * distance, &inner, &outer) - expected).abs() < 1e-5);
        }
    }

    // CODEX: Offset outer circles give different fade widths on opposite sides of the inner circle.
    #[test]
    fn offset_outer_center_changes_each_ray_intersection() {
        let inner = GlobalTransform::from(Transform::from_scale(Vec3::splat(2.0)));
        let outer =
            GlobalTransform::from(Transform::from_xyz(1.0, 0.0, 0.0).with_scale(Vec3::splat(4.0)));
        assert!((strength(Vec3::X * 3.5, &inner, &outer) - 0.5).abs() < 1e-5);
        assert!((strength(Vec3::NEG_X * 2.5, &inner, &outer) - 0.5).abs() < 1e-5);
        assert_eq!(strength(Vec3::NEG_X * 3.1, &inner, &outer), 0.0);
    }

    // CODEX: Blender's global Z becomes Bevy Y; yaw rotates the actual ellipse axes, not its largest radius.
    #[test]
    fn rotated_offset_ellipse_uses_individual_axes() {
        let inner = GlobalTransform::from(Transform::from_scale(Vec3::splat(0.5)));
        let outer = Transform::from_xyz(0.5, 0.0, 0.0).with_scale(Vec3::new(4.0, 3.0, 2.0));
        let point = Vec3::X * 2.0;
        assert!((strength(point, &inner, &GlobalTransform::from(outer)) - 0.625).abs() < 1e-5);
        let turned = outer.with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
        assert!((strength(point, &inner, &GlobalTransform::from(turned)) - 0.25).abs() < 1e-5);
        assert_eq!(
            strength(Vec3::X * 3.0, &inner, &GlobalTransform::from(turned)),
            0.0
        );
        assert!(strength(Vec3::Z * 3.0, &inner, &GlobalTransform::from(turned)) > 0.0);
    }

    // CODEX: Child rotation under nonuniform parent scale produces shear; decomposition must not discard it.
    #[test]
    fn parented_ellipses_preserve_full_affine_transform() {
        let inner = GlobalTransform::from(Transform::from_scale(Vec3::splat(0.5)));
        let outer = Transform::from_xyz(0.3, 0.0, 0.0)
            .with_rotation(Quat::from_rotation_y(0.6))
            .with_scale(Vec3::new(3.0, 2.0, 1.5));
        let point = Vec3::new(1.0, 0.2, 0.3);
        let expected = strength(point, &inner, &GlobalTransform::from(outer));
        assert!(expected > 0.0 && expected < 1.0);
        let parent = GlobalTransform::from(
            Transform::from_xyz(8.0, -2.0, 4.0)
                .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.7, 0.2, -0.4))
                .with_scale(Vec3::new(2.0, 0.5, 3.0)),
        );
        let transformed_inner = parent.mul_transform(inner.compute_transform());
        let transformed_outer = parent.mul_transform(outer);
        assert!(
            (strength(
                parent.transform_point(point),
                &transformed_inner,
                &transformed_outer
            ) - expected)
                .abs()
                < 1e-5
        );
    }

    // CODEX: Flattened markers and partially non-enclosing shapes cannot leak reception beyond the outer shape.
    #[test]
    fn collapsed_and_non_enclosing_outer_shapes_are_safe() {
        let inner = GlobalTransform::IDENTITY;
        let flat = GlobalTransform::from(Transform::from_scale(Vec3::new(4.0, 0.0, 2.0)));
        assert_eq!(strength(Vec3::ZERO, &inner, &flat), 0.0);
        let shifted = GlobalTransform::from(Transform::from_xyz(3.0, 0.0, 0.0));
        assert_eq!(strength(Vec3::ZERO, &inner, &shifted), 0.0);
        assert_eq!(
            strength(Vec3::splat(f32::NAN), &inner, &GlobalTransform::IDENTITY),
            0.0
        );
    }
}
