#define_import_path corn_game::rendering::wind
#import corn_game::{utils::random::randValue, rendering::vertex_io::CornData}


fn apply_wind_acerola(vertex_in: CornData, time: f32) -> CornData {
    /* acerola example */
    let offset = vertex_in.offset;
    let position = vertex_in.position;
    var idHash : f32 = randValue( u32(abs(offset.x * 10000 + offset.y * 100 + offset.z * 0.05 + 2)) );
    idHash = randValue( u32(idHash * 100000) );

    var strength : f32 = cos(time / 3.0) * cos(time / 5.2) / 2 + 0.5; //these two frequencies work well https://www.desmos.com/calculator/023vwitwiq
    strength *= strength;
    //strength = 0.1;

    let weakness = 1 - strength;
    var wind : f32     = cos((offset.x + offset.y) / 2.0 + time  + (idHash * mix(0.0, 8.0, weakness*weakness))) / 2 + 0.5;

    var movement : f32 = wind + mix(-0.5, 0.1, strength); // use strength to modulate minimum deflection (at 0 strength, modulation is symmetric), total range is always 1
    movement *= position.y * position.y; // more sway at top
    movement *= mix(0.2, 1.3, strength) / 10; // use strength to modulate amount of deflection

    let swayVariance : f32 = mix(0.5, 1.0, idHash);
    movement *= swayVariance; // add some randomness per stalk

    var new_p: vec3<f32> = position;
    new_p.x -= movement; // I'm a little surprised this is negative
    new_p.z -= movement;

    // calculate drop in y due to rotation
    new_p.y *= sqrt(1 - pow(movement / position.y, 2.0));
    new_p.y = mix(new_p.y, position.y, abs(position.x) / 10); // calculate position of leaves less *accurate* in order to get a stretch effect
    
    //flutter
    var flutter : f32 = cos(offset.x + offset.y + time*20 * (idHash+.5));
    flutter *= strength * wind * wind * 4;
    flutter *= new_p.y * new_p.y * position.x * position.x / 100;
    new_p.y += flutter;

    var vertex = vertex_in; vertex.position = new_p;
    return vertex;
}

fn apply_wind(vertex_in: CornData, time: f32) -> CornData{
    
}