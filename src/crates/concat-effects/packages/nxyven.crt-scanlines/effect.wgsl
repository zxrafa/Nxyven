struct Params {
    density: f32,
    darkness: f32,
    curvature: f32,
}

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let p = uv - vec2<f32>(0.5);
    let r2 = dot(p, p);
    let k = params.curvature * 0.01 * 0.5;
    let distorted = vec2<f32>(0.5) + p * (1.0 + k * r2);
    
    // Bounds check for barrel distortion
    if (distorted.x < 0.0 || distorted.x > 1.0 || distorted.y < 0.0 || distorted.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    
    let col = sample(distorted);
    let line_freq = max(params.density, 10.0);
    let scan = sin(distorted.y * line_freq * 3.14159265) * 0.5 + 0.5;
    let factor = 1.0 - (params.darkness * 0.01 * 0.45) * (1.0 - scan);
    
    return vec4<f32>(col.rgb * factor, col.a);
}
