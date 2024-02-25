#import bevy_pbr::fragment::pbr_fragment
#import corn_game::corn::vertex_io::{to_vertex_output, CornVertexOutput}
#ifdef PREPASS_PIPELINE
#import bevy_pbr::prepass_io::FragmentOutput
#else
#import bevy_pbr::forward_io::FragmentOutput
#endif

@fragment
fn fragment(in: CornVertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    let standard_in = to_vertex_output(in);
    var out = pbr_fragment(standard_in, is_front);
    #ifndef PREPASS_PIPELINE
    #ifdef CORN_INSTANCED
        let green: f32 = out.color.g;
        let red: f32 = out.color.r;
        let lerp: f32 = f32(in.lod_level)*0.1;
        out.color.g = green*lerp + red*(1.0-lerp);
        out.color.r = red*lerp + green*(1.0-lerp);
        out.color.b = f32(in.lod_level%2);
    #endif
    #endif
    return out;
}