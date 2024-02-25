#define_import_path corn_game::utils::random::noise

#import corn_game::utils::random::{irand1f1, irand2f1, hash4, to_float4, hash2}

// Noise functions.
fn noise(pos: f32) -> f32{ // Lerps betweem neighboring points. smoothStep here is kinda unnecessary for 1d noise, but I have it for now. can be removed if the noise is too slow
    let ipos = floor(pos);
    return mix(irand1f1(ipos), irand1f1(ipos+1.0), smoothStep(0.0, 1.0, fract(p)));
}
fn noise_2d(pos: vec2f) -> f32{// Bilinear Lerp with the factor being put through the smoothstep function to make it more continuos
    let delta = vec2f(0., 1.);
    let ipos = floor(pos);
    let lerp_factor = smoothStep(vec2f(0.), vec2f(1.), fract(pos));
    return mix(mix(irand2f1(ipos).x, irand2f1(ipos + delta.yx).x, lerp_factor.x), mix(irand2f1(ipos + delta.xy).x, irand2f1(ipos + delta.yy).x, lerp_factor.x), lerp_factor.y);
}
fn noise3(p: vec3f) -> f32 {
    let ipos = floor(p); // integer position
    var lerp_factor : vec3f = p - ipos;
    lerp_factor = smoothStep(lerp_factor); // Smooth step fract to make the lerping more continous
    // Generate hashes, start with x, then add in y, then add z
    let x_hash: vec2u = hash2(bitcast<vec2u>(ipos.xx+vec2f(0.0, 1.0)));
    let y_hash: vec4u = hash4(x_hash.xyxy + bitcast<vec4u>(ipos.yyyy+vec4f(0.0, 0.0, 1.0, 1.0)));
    let z_hash_bot: vec4f = to_float4(hash4(y_hash + bitcast<vec4u>(ipos.zzzz)));//Contains the bottom rand values
    let z_hash_top: vec4f = to_float4(hash4(y_hash + bitcast<vec4u>(ipos.zzzz+ 1.0)));// contains the top rand values
    // Trilinear Lerp the 3 axis's into one
    let z_collapsed: vec4f = mix(z_hash_bot, z_hash_top, lerp_factor.z);
    let y_collapsed: vec2f = mix(z_collapsed.xy, z_collapsed.zw, lerp_factor.y);
    let x_collapsed: f32 = mix(y_collapsed.x, y_collapsed.y, lerp_factor.x);
    return x_collapsed;
}