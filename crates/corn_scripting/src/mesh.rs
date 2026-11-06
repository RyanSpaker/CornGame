//! CLAUDE (claude-opus-5-5): meshes scripts build. A script's `Mesh` is a rust `Mesh` kept here by id;
//! every method runs as it's called (no recipe), and numbers cross as one array. Scripts use bevy's
//! names: primitives (`Circle.new(0.1).mesh().resolution(16).build()`) and `Mesh` methods (`merge`,
//! `transformed_by`, `with_inserted_attribute`...; bevy's consume the mesh, these change it and give it
//! back). Given to a `Handle<Mesh>` field (it reads as `mesh:<id>`) a mesh becomes an asset, once: the
//! same script mesh everywhere is the same asset, until it changes.
//!
//! ```js
//! const lamps = Mesh.new();
//! for (const t of places) lamps.merge(lamp.clone().transformed_by(t));
//! spawn({ Mesh3d: lamps });
//! ```

use std::{cell::RefCell, collections::HashMap};

use bevy::prelude::*;
use corn_assets::mesh as shapes;

#[derive(Default)]
struct Meshes {
    next: u32,
    made: HashMap<u32, Made>,
}

#[derive(Default)]
struct Made {
    /// None until something is added
    mesh: Option<Mesh>,
    /// the asset it became, while it hasn't changed since
    asset: Option<Handle<Mesh>>,
}

thread_local! {
    static MESHES: RefCell<Meshes> = RefCell::default();
}

fn with<R>(id: u32, f: impl FnOnce(&mut Made) -> Result<R, String>) -> Result<R, String> {
    MESHES.with_borrow_mut(|m| f(m.made.get_mut(&id).ok_or_else(|| format!("no mesh {id} (dropped?)"))?))
}

fn new(mesh: Option<Mesh>) -> u32 {
    MESHES.with_borrow_mut(|m| {
        m.next += 1;
        m.made.insert(m.next, Made { mesh, asset: None });
        m.next
    })
}

/// merge `part` into the mesh, which changes it
fn merge(made: &mut Made, part: Mesh) -> Result<(), String> {
    made.asset = None;
    match &mut made.mesh {
        Some(mesh) => mesh.merge(&part).map_err(|e| e.to_string()),
        None => {
            made.mesh = Some(part);
            Ok(())
        }
    }
}

fn changed<'m>(made: &'m mut Made, op: &str) -> Result<&'m mut Mesh, String> {
    made.asset = None;
    made.mesh.as_mut().ok_or_else(|| format!("`{op}` needs something in the mesh first"))
}

// CLAUDE
/// the one host call: `op` on mesh `id`, with a name (shapes) and numbers. gives a new mesh's id, a
/// count, or 0
pub(crate) fn host_mesh(op: &str, id: u32, name: &str, n: &[f32]) -> Result<f64, String> {
    let _t = crate::profile::time("mesh");
    let arg = |i: usize| n.get(i).copied().ok_or_else(|| format!("`{op}`: missing argument {}", i + 1));
    match op {
        "new" => Ok(new(None) as f64),
        "shape" => Ok(new(Some(shapes::shape(name, n)?)) as f64),
        "shapes" => Err("use the constructors".into()),
        "drop" => {
            MESHES.with_borrow_mut(|m| m.made.remove(&id));
            Ok(0.0)
        }
        "count_vertices" => with(id, |m| Ok(m.mesh.as_ref().map_or(0, Mesh::count_vertices) as f64)),
        // bevy's own Mesh methods, by their names (the glue keeps the names)
        "clone" => {
            let mesh = with(id, |m| Ok(m.mesh.clone()))?;
            Ok(new(mesh) as f64)
        }
        "merge" => {
            let other = with(arg(0)? as u32, |p| p.mesh.clone().ok_or_else(|| "merging an empty mesh".to_string()))?;
            with(id, |m| merge(m, other)).map(|()| 0.0)
        }
        // translate xyz, rotation xyzw, scale xyz
        "transformed_by" => {
            let transform = Transform {
                translation: Vec3::new(arg(0)?, arg(1)?, arg(2)?),
                rotation: Quat::from_xyzw(arg(3)?, arg(4)?, arg(5)?, arg(6)?).normalize(),
                scale: Vec3::new(arg(7)?, arg(8)?, arg(9)?),
            };
            with(id, |m| changed(m, op).map(|mesh| mesh.transform_by(transform))).map(|()| 0.0)
        }
        "duplicate_vertices" => with(id, |m| changed(m, op).map(Mesh::duplicate_vertices)).map(|()| 0.0),
        "compute_flat_normals" => with(id, |m| changed(m, op)?.try_compute_flat_normals().map_err(|e| e.to_string())).map(|()| 0.0),
        "compute_smooth_normals" => with(id, |m| changed(m, op)?.try_compute_smooth_normals().map_err(|e| e.to_string())).map(|()| 0.0),
        // `name`: POSITION, NORMAL, UV_0 or COLOR; the values flat
        "insert_attribute" => {
            use bevy::mesh::VertexAttributeValues as V;
            let chunks = |k: usize| n.chunks_exact(k);
            let (attribute, values) = match name {
                "POSITION" => (Mesh::ATTRIBUTE_POSITION, V::Float32x3(chunks(3).map(|c| [c[0], c[1], c[2]]).collect())),
                "NORMAL" => (Mesh::ATTRIBUTE_NORMAL, V::Float32x3(chunks(3).map(|c| [c[0], c[1], c[2]]).collect())),
                "UV_0" => (Mesh::ATTRIBUTE_UV_0, V::Float32x2(chunks(2).map(|c| [c[0], c[1]]).collect())),
                "COLOR" => (Mesh::ATTRIBUTE_COLOR, V::Float32x4(chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect())),
                _ => return Err(format!("no attribute `{name}` (POSITION, NORMAL, UV_0, COLOR)")),
            };
            with(id, |m| {
                m.asset = None;
                let mesh = m.mesh.get_or_insert_with(|| Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default()));
                mesh.insert_attribute(attribute, values);
                Ok(0.0)
            })
        }
        "insert_indices" => with(id, |m| {
            m.asset = None;
            let mesh = m.mesh.get_or_insert_with(|| Mesh::new(bevy::mesh::PrimitiveTopology::TriangleList, bevy::asset::RenderAssetUsages::default()));
            mesh.insert_indices(bevy::mesh::Indices::U32(n.iter().map(|&i| i as u32).collect()));
            Ok(0.0)
        }),
        _ => Err(format!("meshes have no `{op}`")),
    }
}

/// a copy of the mesh `mesh:<id>` names. None if `key` isn't one
pub(crate) fn baked(key: &str) -> Option<Result<Mesh, String>> {
    let id: u32 = key.strip_prefix("mesh:")?.parse().ok()?;
    Some(with(id, |made| made.mesh.clone().ok_or_else(|| "an empty mesh".to_string())))
}

// CLAUDE
/// `mesh:<id>` as an asset: made the first time, the same handle after (until the mesh changes)
pub(crate) fn handle(key: &str, assets: &AssetServer) -> Option<Result<Handle<Mesh>, String>> {
    let id: u32 = key.strip_prefix("mesh:")?.parse().ok()?;
    Some(with(id, |made| {
        if let Some(handle) = &made.asset {
            return Ok(handle.clone());
        }
        let mesh = made.mesh.clone().ok_or_else(|| "an empty mesh".to_string())?;
        let handle = assets.add(mesh);
        made.asset = Some(handle.clone());
        Ok(handle)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_in_place() {
        let lamp = host_mesh("shape", 0, "Circle", &[0.1, 8.0]).unwrap() as u32;
        let per_lamp = host_mesh("count_vertices", lamp, "", &[]).unwrap();
        let ring = host_mesh("new", 0, "", &[]).unwrap() as u32;
        for i in 0..4 {
            let a = (i as f32 * 90.0).to_radians();
            let copy = host_mesh("clone", lamp, "", &[]).unwrap() as u32;
            let q = Quat::from_rotation_y(a);
            host_mesh("transformed_by", copy, "", &[a.sin(), 2.0, a.cos(), q.x, q.y, q.z, q.w, 1.0, 1.0, 1.0]).unwrap();
            host_mesh("merge", ring, "", &[copy as f32]).unwrap();
        }
        assert_eq!(host_mesh("count_vertices", ring, "", &[]).unwrap(), per_lamp * 4.0);

        // a quad from attributes and indices
        let quad = host_mesh("new", 0, "", &[]).unwrap() as u32;
        host_mesh("insert_attribute", quad, "POSITION", &[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 0.0]).unwrap();
        host_mesh("insert_attribute", quad, "UV_0", &[0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0]).unwrap();
        host_mesh("insert_indices", quad, "", &[0.0, 1.0, 2.0, 0.0, 2.0, 3.0]).unwrap();
        host_mesh("duplicate_vertices", quad, "", &[]).unwrap();
        host_mesh("compute_flat_normals", quad, "", &[]).unwrap();
        host_mesh("merge", ring, "", &[quad as f32]).unwrap();
        assert_eq!(host_mesh("count_vertices", ring, "", &[]).unwrap(), per_lamp * 4.0 + 6.0);

        assert!(host_mesh("insert_attribute", quad, "TANGENT", &[]).unwrap_err().contains("no attribute"));
        assert!(host_mesh("shape", 0, "Blob", &[]).unwrap_err().contains("no shape"));
        assert!(host_mesh("add", ring, "", &[]).unwrap_err().contains("no `add`"));
        host_mesh("drop", ring, "", &[]).unwrap();
        assert!(host_mesh("count_vertices", ring, "", &[]).is_err());
    }

    // CLAUDE
    /// meshes a script no longer holds are freed (glue.js's FinalizationRegistry; natively, after each call
    /// js_native's `invoke` runs the callbacks V8 posted)
    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn dropped_meshes_are_freed() {
        use crate::{js::Js, with_world_scope, Engine};
        let live = || MESHES.with_borrow(|m| m.made.len());
        let js = Js::new().unwrap();
        let mut world = World::new();
        world.init_resource::<AppTypeRegistry>();
        let source = r#"
exports.kept = Cuboid.new(1, 1, 1).mesh().build();
exports.build = () => { for (let i = 0; i < 100; i++) Cuboid.new(1, 1, 1).mesh().build(); };
exports.idle = () => {};"#;
        with_world_scope(&mut world, || js.load("meshes", source)).unwrap();
        let before = live();
        with_world_scope(&mut world, || js.call("meshes", "build", 1, None, "{}", "null")).unwrap();
        assert_eq!(live(), before + 100);
        js.collect_garbage();
        with_world_scope(&mut world, || js.call("meshes", "idle", 2, None, "{}", "null")).unwrap();
        assert_eq!(live(), before, "the call ran the finalizers; the module's own mesh stays");
    }
}
