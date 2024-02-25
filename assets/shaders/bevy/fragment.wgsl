#define_import_path bevy_pbr::fragment
#import bevy_pbr::{pbr_fragment::pbr_input_from_standard_material, pbr_functions::alpha_discard}

#ifdef PREPASS_PIPELINE
    #import bevy_pbr::{
        prepass_io::{VertexOutput, FragmentOutput}, 
        pbr_deferred_functions::deferred_output
    }
#else
    #import bevy_pbr::{
        forward_io::{VertexOutput, FragmentOutput}, 
        pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing}
    }
#endif

fn pbr_fragment(in: VertexOutput, is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);
    #ifdef PREPASS_PIPELINE
        let out = deferred_output(in, pbr_input);
    #else
        var out: FragmentOutput;
        out.color = apply_pbr_lighting(pbr_input);
        out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    #endif
    return out;
}