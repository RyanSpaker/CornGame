#define_import_path corn_game::utils::random::simplex

#import corn_game::utils::random::{, to_float3, taylor_inv_sqrt3, hash3_seperate}

const transform_factor: vec2f = vec2f(0.5*(sqrt(3.0)-1.0));
const unskew_factor: vec2f = vec2f(0.5 - sqrt(3.0)/6.0);

// 2d Simplex noise, returns a value between -1 and 1
fn snoise_2d(pos: vec2f) -> f32{
    // First transform the position by sending it through the matrix:
    // (1+sqrt(3))/2   (sqrt(3)-1)/2
    // (sqrt(3)-1)/2   (1+sqrt(3))/2
    // This squishes and stretches the diagonals to make the grid squares be made up of equilateral parallelograms. I assume we use this transformation because the eigenvectors are nicely placed on the diagonals
    let skewed_base_pos: vec2f = floor(pos + dot(pos, transform_factor));
    let base_pos: vec2f = skewed_base_pos - dot(unskew_factor, skewed_base_pos);
    let internal_pos = pos - base_pos; // pos relative to base corner
    // Next, we have to determine which of the two equilateral triangles we are in. We start with the base pos's corner, which is always included in the final triangle, along with base_pos + 1 which is also included.
    // If x > y for internal pos, we use base_pos + (1, 0), otherwise we use base_pos + (0, 1)
    let i_corner_x: f32 = step(internal_pos.y, internal_pos.x);
    let far_internal_pos: vec2f = internal_pos - unskew_factor.xx;// pos relative to far corner
    let inter_internal_pos: vec2f = internal_pos - unskew_factor.xx*vec2f(i_corner_x, 1.0-i_corner_x); // pos relative to inter corner
    // Calculate random gradients at each of the 3 corners:
    let grad: vec3u = hash3_seperate(hash3_seperate(bitcast<vec3u>(base_pos.xxx + vec3f(0.0, i_corner_x, 1.0))) + bitcast<vec3u>(base_pos.yyy + vec3f(0.0, 1.0 - i_corner_x, 1.0)));
    let grad_x: vec3f = to_float3(grad);
    let grad_y: vec3f = to_float3(hash3_seperate(grad));
    // Convert gradient directions to gradient samples
    let grad_samples: vec3f = vec3f(
        dot(internal_pos, vec2f(grad_x.x, grad_y.x)), 
        dot(inter_internal_pos, vec2f(grad_x.y, grad_y.y)), 
        dot(far_internal_pos, vec2f(grad_x.z, grad_y.z))
    );
    // Calculate vertex contributions
    var m = max(0.5 - vec3f(dot(internal_pos, internal_pos), dot(inter_internal_pos, inter_internal_pos), dot(far_internal_pos, far_internal_pos)), vec3f(0.0));
    m *= m; m*= m; // to the power 4
    //Normalize Gradients by including the recip(length) to the contribution value
    m*= taylor_inv_sqrt3(grad_x*grad_x + grad_y*grad_y);
    // sum kernel and return noise value
    return dot(m, grad_samples);
}