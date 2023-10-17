use bevy::prelude::*;


pub fn print_frame_rate(
    time: Res<Time>
){
    println!("Frame Rate: {}", time.delta_seconds().recip());
}