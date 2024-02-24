#import bevy_pbr::fragment::pbr_fragment
#ifdef PREPASS_PIPELINE
#import bevy_pbr::prepass_io::FragmentOutput
#else
#import bevy_pbr::forward_io::FragmentOutput
#endif
#import corn_game::{utils::random::{randValue, randNext}}

#ifdef PREPASS_PIPELINE
	#import bevy_pbr::{prepass_io::{Vertex, VertexOutput}, prepass_vertex::standard_vertex}
#else
	#import bevy_pbr::{forward_io::{Vertex, VertexOutput}, standard_vertex::standard_vertex}
#endif

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var seed : f32 = randValue(u32(abs(in.world_position.x) * 12));
    seed += randValue(u32(abs(in.world_position.z) * 12));

    var in2: VertexOutput = in;

    in2.world_normal.x = mix(-1.0, 1.0, randValue(u32(seed * 100000)));
    in2.world_normal.y = mix(-0.2, 1.0, randNext());
    in2.world_normal.z = mix(-1.0, 1.0, randNext());
    if( abs(in.world_position.x) < 100 && abs(in.world_position.z) < 100){
        // var out: FragmentOutput;
        // out.color = vec4(0.0);
        // return out;
        discard;
    }

    var out = pbr_fragment(in2, is_front);
    return out;
}