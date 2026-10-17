struct Params {
    softness: f32,
    invert: f32,
}

fn transition(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let from_col = from_at(uv);
    let to_col = to_at(uv);
    
    var luma = dot(from_col.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    if (params.invert > 0.5) {
        luma = 1.0 - luma;
    }
    
    let soft = max(params.softness * 0.005, 0.01);
    let p = smoothstep(0.0, 1.0, progress);
    let threshold = p * (1.0 + 2.0 * soft) - soft;
    let blend = smoothstep(threshold - soft, threshold + soft, luma);
    
    return mix(to_col, from_col, blend);
}
