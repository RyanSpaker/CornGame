//! CLAUDE (claude-opus-5-5): meshes made from data: a [`MeshRecipe`] is a list of operations, built in
//! rust. Scripts write it with a builder (`mesh().add(Cuboid(1, 2, 1), { translate: [0, 1, 0] })`), scene
//! files with a constructor (`Mesh3d: Cuboid(1, 2, 1)`) or the recipe itself. Shapes are named
//! constructors with the arguments of bevy's `new` ([`shape`]).

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

/// let `Handle<Mesh>` fields take a [`MeshRecipe`]
pub fn register(app: &mut App) {
    app.register_type::<MeshOp>();
    super::register_recipe::<Mesh, MeshRecipe>(app, |recipe| build(&recipe.ops));
}

#[derive(Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Default)]
pub struct MeshRecipe {
    pub ops: Vec<MeshOp>,
}

impl MeshRecipe {
    /// a recipe of one shape
    pub fn shape(name: impl Into<String>, args: Vec<f32>) -> Self {
        MeshRecipe { ops: vec![MeshOp::Shape { name: name.into(), args }] }
    }
}

/// EAS: instuction for claude: this is a bad system. we don't want each operation in js to be made into a recipe and then executed
///      each of these should be a function registered such that it can be called from js on a Mesh
///      the primitives as well, should simply be the bevy types, (get as far as you can just exposing bevy types and functions)
///      each mesh operation should call into rust and execute the change.
/// 
/// one step of a recipe. each adds to, or changes, the mesh built so far
#[derive(Reflect, Clone, Debug, PartialEq)]
pub enum MeshOp {
    /// a primitive by constructor name, merged in
    Shape { name: String, args: Vec<f32> },
    /// `ops` built as a mesh of their own, transformed (rotation in degrees, XYZ), merged in
    Part { ops: Vec<MeshOp>, translate: [f32; 3], rotate: [f32; 3], scale: [f32; 3] },
    /// a grid on XZ, `size` wide, `cells` cells each way. `heights` (row by row, `cells + 1` each way)
    /// may be empty: flat
    Heightfield { size: [f32; 2], cells: [u32; 2], heights: Vec<f32> },
    /// moves every vertex up by fractal noise of its XZ position
    Noise { seed: u32, frequency: f32, amplitude: f32, octaves: u32 },
    SmoothNormals,
    FlatNormals,
    /// a mesh already built (by a script), by its [`bake`] key, merged in. (a `Mesh` itself doesn't keep
    /// its vertices through reflection)
    Baked(u64),
}

// CLAUDE
/// meshes made outside recipes that recipes refer to, by key
static BAKED: std::sync::LazyLock<std::sync::Mutex<(u64, std::collections::HashMap<u64, Mesh>)>> = std::sync::LazyLock::new(Default::default);

/// keeps `mesh` for [`MeshOp::Baked`], giving its key
pub fn bake(mesh: Mesh) -> u64 {
    let mut baked = BAKED.lock().unwrap();
    baked.0 += 1;
    let key = baked.0;
    baked.1.insert(key, mesh);
    key
}

/// builds a mesh from operations
pub fn build(ops: &[MeshOp]) -> Result<Mesh, String> {
    let mut mesh: Option<Mesh> = None;
    let merge = |mesh: &mut Option<Mesh>, part: Mesh| -> Result<(), String> {
        match mesh {
            Some(mesh) => mesh.merge(&part).map_err(|e| e.to_string()),
            None => {
                *mesh = Some(part);
                Ok(())
            }
        }
    };
    let current = |mesh: &mut Option<Mesh>, op: &str| mesh.take().ok_or_else(|| format!("`{op}` needs a mesh before it"));
    for op in ops {
        match op {
            MeshOp::Shape { name, args } => merge(&mut mesh, shape(name, args)?)?,
            MeshOp::Part { ops, translate, rotate, scale } => {
                let [x, y, z] = rotate.map(f32::to_radians);
                let transform = Transform {
                    translation: Vec3::from(*translate),
                    rotation: Quat::from_euler(EulerRot::XYZ, x, y, z),
                    scale: Vec3::from(*scale),
                };
                merge(&mut mesh, build(ops)?.transformed_by(transform))?
            }
            MeshOp::Heightfield { size, cells, heights } => merge(&mut mesh, heightfield(*size, *cells, heights)?)?,
            MeshOp::Baked(key) => {
                let baked = BAKED.lock().unwrap().1.get(key).cloned().ok_or_else(|| format!("no baked mesh {key}"))?;
                merge(&mut mesh, baked)?
            }
            MeshOp::Noise { seed, frequency, amplitude, octaves } => {
                let mut m = current(&mut mesh, "Noise")?;
                displace(&mut m, *seed, *frequency, *amplitude, *octaves)?;
                mesh = Some(m);
            }
            MeshOp::SmoothNormals => {
                let mut m = current(&mut mesh, "SmoothNormals")?;
                m.try_compute_smooth_normals().map_err(|e| e.to_string())?;
                mesh = Some(m);
            }
            MeshOp::FlatNormals => {
                let mut m = current(&mut mesh, "FlatNormals")?;
                m.duplicate_vertices();
                m.try_compute_flat_normals().map_err(|e| e.to_string())?;
                mesh = Some(m);
            }
        }
    }
    mesh.ok_or_else(|| "an empty mesh recipe".into())
}

/// the constructors [`shape`] knows
pub const SHAPES: &[&str] =
    &["Cuboid", "Sphere", "Cylinder", "Capsule3d", "Cone", "Torus", "Plane3d", "Circle", "Rectangle", "Annulus", "ConicalFrustum"];

/// a primitive by constructor: `Cuboid(x, y, z)`, `Sphere(radius)`, `Cylinder(radius, height)`,
/// `Capsule3d(radius, length)`, `Cone(radius, height)`, `Torus(inner, outer)`, `Plane3d(width, depth)`.
/// Flat ones lie in XY facing +Z: `Circle(radius)`, `Rectangle(width, height)`, `Annulus(inner, outer)`.
/// `ConicalFrustum(radius_top, radius_bottom, height)`. Round ones may take a resolution last
/// (`Cylinder(0.1, 2, 8)`)
pub fn shape(name: &str, args: &[f32]) -> Result<Mesh, String> {
    // CLAUDE: the round shapes with a resolution
    match (name, args) {
        ("Cylinder", &[r, h, n]) => return Ok(Cylinder::new(r, h).mesh().resolution(n as u32).build()),
        ("Cone", &[r, h, n]) => return Ok(Cone::new(r, h).mesh().resolution(n as u32).build()),
        ("Circle", &[r, n]) => return Ok(Circle::new(r).mesh().resolution(n as u32).build()),
        ("Annulus", &[inner, outer, n]) => return Ok(Annulus::new(inner, outer).mesh().resolution(n as u32).build()),
        ("ConicalFrustum", &[top, bottom, h, n]) => {
            let frustum = ConicalFrustum { radius_top: top, radius_bottom: bottom, height: h };
            return Ok(frustum.mesh().resolution(n as u32).build());
        }
        _ => {}
    }
    let wrong = |expected: &str| format!("`{name}` takes ({expected}), got {} values", args.len());
    Ok(match (name, args) {
        ("Cuboid", &[x, y, z]) => Cuboid::new(x, y, z).mesh().build(),
        ("Cuboid", _) => return Err(wrong("x, y, z")),
        ("Sphere", &[r]) => Sphere::new(r).mesh().build(),
        ("Sphere", _) => return Err(wrong("radius")),
        ("Cylinder", &[r, h]) => Cylinder::new(r, h).mesh().build(),
        ("Cylinder", _) => return Err(wrong("radius, height")),
        ("Capsule3d", &[r, l]) => Capsule3d::new(r, l).mesh().build(),
        ("Capsule3d", _) => return Err(wrong("radius, length")),
        ("Cone", &[r, h]) => Cone::new(r, h).mesh().build(),
        ("Cone", _) => return Err(wrong("radius, height")),
        ("Torus", &[inner, outer]) => Torus::new(inner, outer).mesh().build(),
        ("Torus", _) => return Err(wrong("inner, outer")),
        ("Plane3d", &[w, d]) => Plane3d::default().mesh().size(w, d).build(),
        ("Plane3d", _) => return Err(wrong("width, depth")),
        ("Circle", &[r]) => Circle::new(r).mesh().build(),
        ("Circle", _) => return Err(wrong("radius")),
        ("Rectangle", &[w, h]) => Rectangle::new(w, h).mesh().build(),
        ("Rectangle", _) => return Err(wrong("width, height")),
        ("Annulus", &[inner, outer]) => Annulus::new(inner, outer).mesh().build(),
        ("Annulus", _) => return Err(wrong("inner, outer")),
        ("ConicalFrustum", &[top, bottom, h]) => ConicalFrustum { radius_top: top, radius_bottom: bottom, height: h }.mesh().build(),
        ("ConicalFrustum", _) => return Err(wrong("radius_top, radius_bottom, height")),
        _ => return Err(format!("no shape `{name}` ({})", SHAPES.join(", "))),
    })
}

fn heightfield(size: [f32; 2], cells: [u32; 2], heights: &[f32]) -> Result<Mesh, String> {
    let [cx, cz] = cells.map(|c| c.max(1) as usize);
    let (nx, nz) = (cx + 1, cz + 1);
    if !heights.is_empty() && heights.len() != nx * nz {
        return Err(format!("{} heights for a {cx}x{cz} grid, which has {} corners", heights.len(), nx * nz));
    }
    let mut positions = Vec::with_capacity(nx * nz);
    let mut uvs = Vec::with_capacity(nx * nz);
    for z in 0..nz {
        for x in 0..nx {
            let (u, v) = (x as f32 / cx as f32, z as f32 / cz as f32);
            let y = heights.get(z * nx + x).copied().unwrap_or(0.0);
            positions.push([(u - 0.5) * size[0], y, (v - 0.5) * size[1]]);
            uvs.push([u, v]);
        }
    }
    let mut indices = Vec::with_capacity(cx * cz * 6);
    for z in 0..cz {
        for x in 0..cx {
            let i = (z * nx + x) as u32;
            let (right, down) = (i + 1, i + nx as u32);
            indices.extend([i, down, right, right, down, down + 1]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices));
    mesh.try_compute_smooth_normals().map_err(|e| e.to_string())?;
    Ok(mesh)
}

fn displace(mesh: &mut Mesh, seed: u32, frequency: f32, amplitude: f32, octaves: u32) -> Result<(), String> {
    use bevy::mesh::VertexAttributeValues;
    let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION) else {
        return Err("the mesh has no positions to move".into());
    };
    for p in positions.iter_mut() {
        p[1] += amplitude * fbm(seed, p[0] * frequency, p[2] * frequency, octaves.max(1));
    }
    if mesh.indices().is_some() {
        mesh.try_compute_smooth_normals().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// fractal value noise, about -1..1
fn fbm(seed: u32, x: f32, z: f32, octaves: u32) -> f32 {
    let (mut sum, mut scale, mut weight, mut total) = (0.0, 1.0, 1.0, 0.0);
    for octave in 0..octaves {
        sum += weight * value_noise(seed.wrapping_add(octave), x * scale, z * scale);
        total += weight;
        scale *= 2.0;
        weight *= 0.5;
    }
    sum / total
}

fn value_noise(seed: u32, x: f32, z: f32) -> f32 {
    let (x0, z0) = (x.floor(), z.floor());
    let (fx, fz) = (x - x0, z - z0);
    let (sx, sz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let corner = |dx: i32, dz: i32| hash(seed, x0 as i32 + dx, z0 as i32 + dz);
    let top = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * sx;
    let bottom = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * sx;
    top + (bottom - top) * sz
}

/// -1..1, the same for the same inputs everywhere
fn hash(seed: u32, x: i32, z: i32) -> f32 {
    let mut h = seed ^ (x as u32).wrapping_mul(0x27d4_eb2d) ^ (z as u32).wrapping_mul(0x1656_67b1);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h as f32 / u32::MAX as f32 * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vertices(mesh: &Mesh) -> usize {
        mesh.count_vertices()
    }

    #[test]
    fn builds() {
        let lamp = build(&[
            MeshOp::Part { ops: vec![MeshOp::Shape { name: "Cylinder".into(), args: vec![0.08, 3.0] }], translate: [0.0, 1.5, 0.0], rotate: [0.0; 3], scale: [1.0; 3] },
            MeshOp::Part { ops: vec![MeshOp::Shape { name: "Sphere".into(), args: vec![0.25] }], translate: [0.0, 3.0, 0.0], rotate: [0.0; 3], scale: [1.0; 3] },
            MeshOp::Shape { name: "Cuboid".into(), args: vec![0.6, 0.05, 0.6] },
        ])
        .unwrap();
        let parts = shape("Cylinder", &[0.08, 3.0]).unwrap().count_vertices()
            + shape("Sphere", &[0.25]).unwrap().count_vertices()
            + shape("Cuboid", &[0.6, 0.05, 0.6]).unwrap().count_vertices();
        assert_eq!(vertices(&lamp), parts);

        let ground = build(&[
            MeshOp::Heightfield { size: [10.0, 10.0], cells: [4, 4], heights: vec![] },
            MeshOp::Noise { seed: 7, frequency: 0.5, amplitude: 2.0, octaves: 3 },
        ])
        .unwrap();
        assert_eq!(vertices(&ground), 25);
        // the same seed makes the same mesh
        let again = build(&[
            MeshOp::Heightfield { size: [10.0, 10.0], cells: [4, 4], heights: vec![] },
            MeshOp::Noise { seed: 7, frequency: 0.5, amplitude: 2.0, octaves: 3 },
        ])
        .unwrap();
        assert_eq!(ground.attribute(Mesh::ATTRIBUTE_POSITION), again.attribute(Mesh::ATTRIBUTE_POSITION));

        assert!(shape("Cuboid", &[1.0]).unwrap_err().contains("takes (x, y, z)"));
        assert!(build(&[MeshOp::SmoothNormals]).unwrap_err().contains("needs a mesh"));
    }
}
