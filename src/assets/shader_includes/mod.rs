use bevy::{prelude::*, asset::load_internal_asset};

pub const CORN_COMMON: Handle<Shader> = Handle::weak_from_u128(264555199423856789);
pub struct ShaderIncludesPlugin{}
impl Plugin for ShaderIncludesPlugin{
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            CORN_COMMON,
            "corn_common.wgsl",
            Shader::from_wgsl
        );
    }
}