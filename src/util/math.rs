// Lerps from a to b with percent r
pub fn lerp<T>(a: T, b: T, r: f32) -> T
where
    T: std::ops::Sub<Output = T>
        + std::ops::Mul<f32, Output = T>
        + std::ops::Add<Output = T> + Clone,
{
    a.clone() + (b - a) * r
}

// exp ease to
pub trait ExpEaseTo {
    fn exp_ease_to(&mut self, target: Self, rate: f32, delta_time: f32) -> Self;
}

impl<T> ExpEaseTo for T
where
    T: std::ops::Sub<Output = T>
        + std::ops::Mul<f32, Output = T>
        + std::ops::AddAssign
        + Copy,
{
    fn exp_ease_to(&mut self, target: Self, rate: f32, delta_time: f32) -> Self {
        let diff = target - *self;
        let change = diff * (1.0 - (-rate * delta_time).exp());
        *self += change;
        *self
    }
}