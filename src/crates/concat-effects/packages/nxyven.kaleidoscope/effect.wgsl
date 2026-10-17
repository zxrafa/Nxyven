struct Params {
    segments: f32,
    rotation: f32,
    zoom: f32,
}

fn effect(uv: vec2<f32>) -> vec4<f32> {
    let aspect = frame.size.x / frame.size.y;
    let p = (uv - vec2<f32>(0.5)) * vec2<f32>(aspect, 1.0);
    let r = length(p);
    let pi2 = 6.283185307;
    let raw_a = atan2(p.y, p.x) + params.rotation * 0.01 * pi2;
    let segs = max(round(params.segments), 2.0);
    let slice = pi2 / segs;
    let step_idx = floor(raw_a / slice + 1000.0);
    var a = (raw_a + 1000.0 * slice) - floor((raw_a + 1000.0 * slice) / slice) * slice;
    if ((step_idx % 2.0) >= 0.5) {
        a = slice - a;
    }
    let z = max(params.zoom * 0.01, 0.2);
    let folded_p = vec2<f32>(cos(a), sin(a)) * r / z;
    let sample_uv = vec2<f32>(0.5) + folded_p / vec2<f32>(aspect, 1.0);
    return sample(clamp(sample_uv, vec2<f32>(0.0), vec2<f32>(1.0)));
}
