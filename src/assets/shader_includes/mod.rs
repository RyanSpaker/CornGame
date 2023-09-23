use bevy::{prelude::*, asset::load_internal_asset, reflect::TypeUuid};

pub const CORN_COMMON: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 264555199423856789);
pub const CORN_LOD: HandleUntyped =
    HandleUntyped::weak_from_u64(Shader::TYPE_UUID, 264555199423852354);

pub struct ShaderIncludesPlugin{}
impl Plugin for ShaderIncludesPlugin{
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            CORN_COMMON,
            "corn_common.wgsl",
            Shader::from_wgsl
        );
        load_internal_asset!(
            app,
            CORN_LOD,
            "lod_count_override.wgsl",
            Shader::from_wgsl
        );
    }
}