//! CLAUDE (claude-sonnet-5-5) 2026-11-05: build the post-it's pipelines during loading, not at pickup/placement.
//!
//! WHY: precompile.rs builds a mesh's pipelines when it spawns, but two post-it materials are created only
//! when used: the held `PaperMaterial` copy (pickup, post_it::copy_paper) and the typed-ink material and mesh
//! (placement, post_it::render_text). Their pipelines were therefore first built on the very frame they were
//! first drawn: a visible freeze on web, where compiles are synchronous. This requests them ahead of time
//! with `PrecompileQueue::request`, from the same authored sheets and materials, through the same builders
//! (`paper_material`, `ink_material`, `ink_mesh`) the real code uses, so the pipeline keys match.
//!
//! The requested assets are never drawn; they are kept alive here because precompile waits for their mesh and
//! material to reach the GPU before it builds.
//!
//! TODO: instead of at load, request these when the player looks at the post-its (the stack's interaction
//! range or view), so loading doesn't build pipelines for something that may never be touched. Pipeline
//! building is synchronous on web, so that needs the build staggered over frames before the player can reach
//! for a note.
use bevy::{
    asset::RenderAssetUsages,
    gltf::GltfMaterialName,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use crate::{
    post_it::{PaperSettings, PostItStack},
    post_it_material::{ink_mask, ink_material, paper_material, PaperMaterial},
    post_it_text::ink_mesh,
    precompile::PrecompileQueue,
};

// CLAUDE: prototype assets kept alive after requesting; `done` once everything has been requested
#[derive(Resource, Default)]
struct Warmed {
    done: bool,
    keep: Vec<UntypedHandle>,
}

// CLAUDE
pub fn plugin(app: &mut App) {
    app.init_resource::<Warmed>()
        .add_systems(Update, warm.run_if(|warmed: Res<Warmed>| !warmed.done));
}

// CLAUDE: once the authored sheets and the handwriting material have loaded, request the paper and ink pipelines.
// Retries every frame until they have.
fn warm(
    mut warmed: ResMut<Warmed>,
    stacks: Query<Entity, With<PostItStack>>,
    names: Query<&Name>,
    children: Query<&Children>,
    primitives: Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>)>,
    authored: Query<(&GltfMaterialName, &MeshMaterial3d<StandardMaterial>)>,
    settings: Res<PaperSettings>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    mut paper: ResMut<Assets<PaperMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut queue: ResMut<PrecompileQueue>,
) {
    let Some(stack) = stacks.iter().next() else { return };
    let Some(ink_source) = authored.iter().find(|(name, _)| name.0 == "postit_handwritten_ink")
        .and_then(|(_, handle)| standard.get(&handle.0)).cloned() else { return };
    // copy_paper copies the sheet entity and everything under it; the sheets are the stack's postit1..4
    let sheets = children.iter_descendants(stack)
        .filter(|&e| names.get(e).is_ok_and(|n| n.as_str().strip_prefix("postit").is_some_and(|rest| rest.parse::<u32>().is_ok())));
    let mut parts = Vec::new();
    for sheet in sheets {
        for e in std::iter::once(sheet).chain(children.iter_descendants(sheet)) {
            let Ok((mesh, material)) = primitives.get(e) else { continue };
            let (Some(mesh_asset), Some(material_asset)) = (meshes.get(&mesh.0), standard.get(&material.0)) else { return };
            parts.push((mesh.0.clone(), mesh_asset.clone(), material_asset.clone()));
        }
    }
    if parts.is_empty() { return; }

    // a 1x1 stand-in for the rasterized text: only the texture's presence shapes the pipeline
    let image = images.add(ink_mask(Image::new_fill(
        Extent3d { width: 1, height: 1, depth_or_array_layers: 1 }, TextureDimension::D2,
        &[0, 0, 0, 0], TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default())));
    let ink = standard.add(ink_material(ink_source, image.clone()));
    warmed.keep.extend([image.untyped(), ink.clone().untyped()]);
    for (mesh_handle, mesh, material) in parts {
        // pickup: the held paper copy, on the sheet's own mesh
        let held = paper.add(paper_material(material));
        queue.request::<PaperMaterial>(mesh_handle.id(), held.id(), true);
        warmed.keep.extend([mesh_handle.untyped(), held.untyped()]);
        // placement: the ink mesh built from that sheet mesh (its bounds stand in for the paper's)
        let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION).and_then(|a| a.as_float3()) else { continue };
        let (min, max) = positions.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p))));
        if let Some(ink_mesh) = ink_mesh(&mesh, min, max, settings.ink_gap) {
            let ink_mesh = meshes.add(ink_mesh);
            queue.request::<StandardMaterial>(ink_mesh.id(), ink.id(), true);
            warmed.keep.push(ink_mesh.untyped());
        }
    }
    warmed.done = true;
}
