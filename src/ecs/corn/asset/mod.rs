use bevy::{asset::{AssetLoader, AsyncReadExt}, gltf::GltfError, prelude::*};

pub struct CornAssetPlugin;
impl Plugin for CornAssetPlugin{
    fn build(&self, _app: &mut App) {
        
    }
}

#[derive(Default, Debug, Clone, TypePath, Asset)]
pub struct RawCornAsset;

pub struct CornGltfLoader;
impl AssetLoader for CornGltfLoader{
    type Asset = RawCornAsset;

    type Settings = ();

    type Error = GltfError;

    fn load<'a>(
        &'a self,
        reader: &'a mut bevy::asset::io::Reader,
        settings: &'a Self::Settings,
        load_context: &'a mut bevy::asset::LoadContext,
    ) -> bevy::utils::BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes).await?;
            let gltf = gltf::Gltf::from_slice(bytes.as_slice())?;
            return Ok(RawCornAsset);
        })
    }

    fn extensions(&self) -> &[&str] {
        &["gltf", "glb"]
    }
}