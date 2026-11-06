//! CLAUDE (claude-opus-5-5): mip chains for jpg/png images at load time. 2x2 box filter per level, from the one
//! above: sRGB images in linear light, normal maps as renormalized vectors, other data as stored.

use bevy::{
    image::Image,
    render::render_resource::{TextureDimension, TextureFormat},
};

use crate::manifest::Role;

/// give `image` a full mip chain, if it is a plain 2D RGBA8 image with only its top level. Returns whether it did
pub fn generate(image: &mut Image, role: Role) -> bool {
    let desc = &image.texture_descriptor;
    let srgb = match desc.format {
        TextureFormat::Rgba8UnormSrgb => true,
        TextureFormat::Rgba8Unorm => false,
        _ => return false,
    };
    if desc.mip_level_count != 1
        || desc.dimension != TextureDimension::D2
        || desc.size.depth_or_array_layers != 1
    {
        return false;
    }
    let (w, h) = (desc.size.width, desc.size.height);
    let Some(top) = image.data.as_ref() else { return false };
    if top.len() != (w * h * 4) as usize {
        return false;
    }

    let filter = Filter::new(if role == Role::Normal { Mode::Normal } else if srgb { Mode::Srgb } else { Mode::Linear });
    let count = 32 - w.max(h).leading_zeros();
    // whole chain: 4/3 of the top level, plus rounding
    let mut data = Vec::with_capacity(top.len() * 4 / 3 + 64);
    data.extend_from_slice(top);
    let (mut start, mut lw, mut lh) = (0usize, w, h);
    for _ in 1..count {
        let (nw, nh) = ((lw / 2).max(1), (lh / 2).max(1));
        let next_start = data.len();
        filter.halve(&mut data, start, lw, lh, nw, nh);
        (start, lw, lh) = (next_start, nw, nh);
    }
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = count;
    true
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Srgb,
    Linear,
    Normal,
}

struct Filter {
    mode: Mode,
    /// sRGB byte -> linear
    to_linear: [f32; 256],
    /// linear * 4095 -> sRGB byte
    to_srgb: Vec<u8>,
}

impl Filter {
    fn new(mode: Mode) -> Self {
        let to_linear = std::array::from_fn(|v| {
            let v = v as f32 / 255.0;
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        });
        let to_srgb = (0..4096)
            .map(|i| {
                let v = i as f32 / 4095.0;
                let s = if v <= 0.0031308 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 };
                (s * 255.0).round().clamp(0.0, 255.0) as u8
            })
            .collect();
        Self { mode, to_linear, to_srgb }
    }

    /// append the `nw`x`nh` level made from the `lw`x`lh` one at `data[start..]`
    fn halve(&self, data: &mut Vec<u8>, start: usize, lw: u32, lh: u32, nw: u32, nh: u32) {
        let (lw, lh) = (lw as usize, lh as usize);
        for y in 0..nh as usize {
            let (y0, y1) = ((y * 2).min(lh - 1), (y * 2 + 1).min(lh - 1));
            for x in 0..nw as usize {
                let (x0, x1) = ((x * 2).min(lw - 1), (x * 2 + 1).min(lw - 1));
                let px = |xx: usize, yy: usize| start + (yy * lw + xx) * 4;
                let p = [px(x0, y0), px(x1, y0), px(x0, y1), px(x1, y1)];
                let out = self.average(data, p);
                data.extend_from_slice(&out);
            }
        }
    }

    fn average(&self, data: &[u8], p: [usize; 4]) -> [u8; 4] {
        let mut sum = [0f32; 4];
        for &i in &p {
            for c in 0..3 {
                let v = data[i + c];
                sum[c] += match self.mode {
                    Mode::Srgb => self.to_linear[v as usize],
                    Mode::Linear => v as f32 / 255.0,
                    Mode::Normal => v as f32 / 127.5 - 1.0,
                };
            }
            sum[3] += data[i + 3] as f32 / 255.0;
        }
        let avg = sum.map(|s| s / 4.0);
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        let rgb = match self.mode {
            Mode::Srgb => {
                let e = |v: f32| self.to_srgb[(v.clamp(0.0, 1.0) * 4095.0).round() as usize];
                [e(avg[0]), e(avg[1]), e(avg[2])]
            }
            Mode::Linear => [q(avg[0]), q(avg[1]), q(avg[2])],
            Mode::Normal => {
                let len = (avg[0] * avg[0] + avg[1] * avg[1] + avg[2] * avg[2]).sqrt();
                let n = if len > 1e-6 { [avg[0] / len, avg[1] / len, avg[2] / len] } else { [0.0, 0.0, 1.0] };
                n.map(|v| q(v * 0.5 + 0.5))
            }
        };
        [rgb[0], rgb[1], rgb[2], q(avg[3])]
    }
}
