struct Params {
    angle: f32,
    feather: f32,
}

fn transition(uv: vec2<f32>, progress: f32) -> vec4<f32> {
    let rad = params.angle * 0.0174532925;
    let dir = vec2<f32>(cos(rad), sin(rad));
    
    let aspect = frame.size.x / frame.size.y;
    let centered = (uv - vec2<f32>(0.5)) * vec2<f32>(aspect, 1.0);
    let proj = dot(centered, dir);
    
    let max_extent = length(vec2<f32>(aspect, 1.0) * 0.5);
    let norm_proj = proj / max_extent * 0.5 + 0.5;
    
    let p = smoothstep(0.0, 1.0, progress);
    let soft = max(params.feather * 0.003, 0.002);
    let m = smoothstep(p - soft, p + soft, norm_proj);
    
    return mix(to_at(uv), from_at(uv), m);
}
