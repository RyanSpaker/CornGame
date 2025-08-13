//! fake positional audio
//! actually better than true positional since we can directly control the falloff.
//! though the directional part would be nice

// paired with Transform scale
struct SoundRange{
    falloff: EaseFunction,
}

// uses a grid to fake it.
struct Fake{
    size: f32
}