//! tools to spawn scarecrows randomly in the world
//! TODO refactor into generic spawn/paint utils

use bevy::prelude::*;


pub fn plugin(app: &mut App) {
    // TODO: better way to conditionally run once, at specific time, from startup
    app.add_systems(
        Update,
        crate::Cli::runnable(spawn_scarecrows)
            .run_if(run_once)
            .run_if(|query: Query<&super::corn::CornField>| !query.is_empty()),
    );
}

pub fn spawn_scarecrows(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    // query: Query<&super::corn::CornField>,
) {
    let bounds = Rect::from_center_half_size(Vec2::ZERO, Vec2::splat(75.0));
    let num_scarecrows = 50;

    for _ in 0..num_scarecrows {
        let x = rand::random_range(bounds.min.x..bounds.max.x);
        let z = rand::random_range(bounds.min.y..bounds.max.y);
        let position = Vec3::new(x, rand::random_range(-0.5..0.0), z);

        commands.spawn((
            Name::from("Scarecrow"),
            Transform::from_translation(position).with_rotation(Quat::from_axis_angle(
                Vec3::new(rand::random::<f32>() * 0.2, 1.0, rand::random::<f32>() * 0.2).normalize(),
                rand::random::<f32>(),
            )),
            // TODO add scarecrow model/mesh/material
            // LoadScene::new("models/scarecrow.glb#Scene0"),
            SceneRoot(
                asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/scarecrow.glb")),
            ),
        ));
    }
}
