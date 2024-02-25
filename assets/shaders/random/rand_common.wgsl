#define_import_path corn_game::utils::random


// Helper Functions
fn mod289(x: f32) -> f32 { return x - floor(x * (1. / 289.)) * 289.; }
fn mod289_2(x: vec2f) -> vec2f { return x - floor(x * (1. / 289.)) * 289.; }
fn mod289_3(x: vec3f) -> vec3f { return x - floor(x * (1. / 289.)) * 289.; }
fn mod289_4(x: vec4f) -> vec4f { return x - floor(x * (1. / 289.)) * 289.; }

fn perm(x: f32) -> f32 { return mod289(((x * 34.) + 10.) * x); }
fn perm2(x: vec2f) -> vec2f { return mod289(((x * 34.) + 10.) * x); }
fn perm3(x: vec3f) -> vec3f { return mod289(((x * 34.) + 10.) * x); }
fn perm4(x: vec4f) -> vec4f { return mod289(((x * 34.) + 10.) * x); }
// fast Low accuracy inv sqrt approximation
fn taylor_inv_sqrt(r: f32) -> f32 { return 1.79284291400159 - 0.85373472095314 * r; }
fn taylor_inv_sqrt2(r: vec2f) -> vec2f { return 1.79284291400159 - 0.85373472095314 * r; }
fn taylor_inv_sqrt3(r: vec3f) -> vec3f { return 1.79284291400159 - 0.85373472095314 * r; }
fn taylor_inv_sqrt4(r: vec4f) -> vec4f { return 1.79284291400159 - 0.85373472095314 * r; }

// Hashing functions. PCG hash
fn hash(p: u32) -> u32{
    var h = p * 747796405u + 2891336453u;
    h = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    return (h >> 22u) ^ h;
}
fn hash2(p: vec2u) -> vec2u{
    var v = p * 1664525u + 1013904223u;
    v.x += v.y * 1664525u; v.y += v.x * 1664525u;
    v ^= v >> vec2<u32>(16u);
    v.x += v.y * 1664525u; v.y += v.x * 1664525u;
    v ^= v >> vec2<u32>(16u);
    return v;
}
fn hash3(p: vec3u) -> vec3u{
    var v = p * 1664525u + 1013904223u;
    v.x += v.y*v.z; v.y += v.z*v.x; v.z += v.x*v.y;
    v ^= v >> vec3<u32>(16u);
    v.x += v.y*v.z; v.y += v.z*v.x; v.z += v.x*v.y;
    return v;
}
fn hash3_seperate(p: vec3u) -> vec3u{
    return vec3u(hash(p.x), hash(p.y), hash(p.z));
}
fn hash4(p: vec4u) -> vec4u{
    var v = p * 1664525u + 1013904223u;
    v.x += v.y*v.w; v.y += v.z*v.x; v.z += v.x*v.y; v.w += v.y*v.z;
    v ^= v >> vec4<u32>(16u);
    v.x += v.y*v.w; v.y += v.z*v.x; v.z += v.x*v.y; v.w += v.y*v.z;
    return v;
}

// Turn u32 into float 0..1
fn to_float(x: u32) -> f32{
    return f32(x) / f32(0xffffffff);
}
fn to_float2(x: u32, y: u32) -> vec2f{
    return vec2f(f32(x), f32(y))/f32(0xffffffff);
}
fn to_float3(x: u32, y: u32, z: u32) -> vec3f{
    return vec2f(f32(x), f32(y), f32(z))/f32(0xffffffff);
}
fn to_float4(x: u32, y: u32, z: u32, w: u32) -> vec4f{
    return vec2f(f32(x), f32(y), f32(z), f32(w))/f32(0xffffffff);
}
// Turn multiple random u32's into one
fn collapse2(v: vec2u) -> u32{
    return v.x^v.y;
}
fn collapse3(v: vec3u) -> u32{
    return v.x^v.y^v.z;
}
fn collapse4(v: vec4u) -> u32{
    return v.x^v.y^v.z^v.w;
}

//Rand Generators:
var<private> rng_state: u32;

fn xorshift() -> u32 {
    rng_state ^= (rng_state << 13);
    rng_state ^= (rng_state >> 17);
    rng_state ^= (rng_state << 5);

    return rng_state;
}

fn next_rand() -> f32{
    return to_float(xorshift());
}

fn rand(seed: u32) -> f32{
    rng_state = hash(seed);
    return to_float(rng_state);
}

// [i]rand[a][b]: 
// i: immediate, doesnt set rng_state. 
// a: input size, append an f if the input is a float. 
// b: output size, append a u if the output is u32

fn irand1f1(seed: f32) -> f32{
    return to_float(hash(bitcast<u32>(seed)));
}

fn irand2f1(seed: vec2f) -> f32{
    return to_float(collapse2(hash2(bitcast<vec2u>(seed))));
}
fn irand3f1(seed: vec3f) -> f32{
    return to_float(collapse3(hash3(bitcast<vec3u>(seed))));
}