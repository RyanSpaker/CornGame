// Stolen from acerola https://github.com/GarrettGunnell/Grass/blob/main/Assets/Resources/Random.cginc
// NOTE: there is also https://github.com/bevyengine/bevy/blob/ac6a4ff386df78ed0e66dba70860c9d16da81bbe/crates/bevy_pbr/src/render/utils.wgsl#L20

#define_import_path corn_game::utils::random

var<private> rng_state: u32;

//Hash invented by Thomas Wang
fn wang_hash(seed : u32) {
    rng_state = (seed ^ 61) ^ (seed >> 16);
    rng_state = rng_state * 9;
    rng_state = rng_state ^ (rng_state >> 4);
    rng_state = rng_state * 0x27d4eb2d;
    rng_state = rng_state ^ (rng_state >> 15);
}

//Xorshift algorithm from George Marsaglia's paper
fn rand_xorshift() -> u32{
    rng_state ^= (rng_state << 13);
    rng_state ^= (rng_state >> 17);
    rng_state ^= (rng_state << 5);

    return rng_state;
}

fn randNext() -> f32 {
    return f32(rand_xorshift())* (1.0 / 4294967296.0);
}

fn initRand(seed : u32){
    wang_hash(seed);
}

fn randValue(seed : u32) -> f32 {
    initRand(seed);
    return randNext();
}