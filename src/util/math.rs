// Lerps from a to b with percent r
pub fn lerp(a: f32, b:f32, r: f32) -> f32{
    a + (b-a)*r
}