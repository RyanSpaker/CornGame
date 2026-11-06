//! CODEX (GPT-6): equal-power stereo panning and inverse-distance attenuation, mirrored in voice.js.
use serde::{Deserialize, Serialize};
use crate::Pose;

/// CODEX: expose tuning independently of any renderer or audio backend.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SpatialSettings {
    pub volume: f32,
    pub reference_distance: f32,
    pub max_distance: f32,
    pub rolloff: f32,
}

// CODEX: metre-scale cabin defaults; max_distance is an explicit hard audible range on both targets.
impl Default for SpatialSettings {
    // CODEX: Supply metre-scale defaults shared by native and browser clients.
    fn default() -> Self {
        Self {
            volume: 1.,
            reference_distance: 1.,
            max_distance: 30.,
            rolloff: 1.,
        }
    }
}

// CODEX: keep hostile signaling values out of the mixer, including NaN and degenerate orientation.
pub fn valid_pose(pose: Pose) -> bool {
    pose.position
        .iter()
        .chain(pose.forward.iter())
        .chain(pose.up.iter())
        .all(|v| v.is_finite() && v.abs() <= 1_000_000.)
}

// CODEX: sanitize public tuning on both entry points; never let a zero reference distance divide by zero.
pub fn sanitize(settings: SpatialSettings) -> SpatialSettings {
    let finite = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
    let reference_distance = finite(settings.reference_distance, 1.).clamp(0.01, 1_000_000.);
    SpatialSettings {
        volume: finite(settings.volume, 1.).clamp(0., 4.),
        reference_distance,
        max_distance: finite(settings.max_distance, 30.).clamp(reference_distance, 1_000_000.),
        rolloff: finite(settings.rolloff, 1.).clamp(0., 10.),
    }
}

// CODEX: forward cross up is listener-right for Bevy's right-handed coordinate convention.
pub fn gains(listener: Pose, source: [f32; 3], settings: SpatialSettings) -> [f32; 2] {
    let s = sanitize(settings);
    let delta = std::array::from_fn::<_, 3, _>(|i| source[i] - listener.position[i]);
    let distance = delta.iter().map(|v| v * v).sum::<f32>().sqrt();
    if !distance.is_finite() || distance >= s.max_distance {
        return [0., 0.];
    }
    let f = listener.forward;
    let u = listener.up;
    let right = [
        f[1] * u[2] - f[2] * u[1],
        f[2] * u[0] - f[0] * u[2],
        f[0] * u[1] - f[1] * u[0],
    ];
    let length = right.iter().map(|v| v * v).sum::<f32>().sqrt();
    let pan = if distance > 0.0001 && length > 0.0001 {
        (delta.iter().zip(right).map(|(a, b)| a * b).sum::<f32>() / (distance * length))
            .clamp(-1., 1.)
    } else {
        0.
    };
    let gain = s.volume * s.reference_distance
        / (s.reference_distance
            + s.rolloff * (distance.max(s.reference_distance) - s.reference_distance));
    let angle = (pan + 1.) * std::f32::consts::FRAC_PI_4;
    [gain * angle.cos(), gain * angle.sin()]
}

#[cfg(test)]
// CODEX: acoustic invariants catch coordinate/sign mistakes and attenuation mismatches.
mod tests {
    use super::*;
    #[test]
    // CODEX: Verify directional and attenuation invariants independent of the audio device.
    fn direction_distance_and_rotation() {
        let p = Pose::default();
        let s = SpatialSettings::default();
        let right = gains(p, [1., 0., 0.], s);
        assert!(right[1] > 0.99 && right[0].abs() < 0.001);
        let left = gains(p, [-1., 0., 0.], s);
        assert!(left[0] > 0.99 && left[1].abs() < 0.001);
        let near = gains(p, [0., 0., -1.], s);
        let far = gains(p, [0., 0., -10.], s);
        assert!((far[0] / near[0] - 0.1).abs() < 0.001);
        assert_eq!(gains(p, [0., 0., -30.], s), [0., 0.]);
        let turned = Pose {
            forward: [0., 0., 1.],
            ..p
        };
        assert!(gains(turned, [1., 0., 0.], s)[0] > 0.99);
    }
}
