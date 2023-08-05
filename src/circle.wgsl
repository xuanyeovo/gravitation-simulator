struct CircleData {
    center: vec3<f32>,
    radius: f32,
}

struct BasicUniform {
    aspect_ratio: f32,
    scale: f32,
    camera_coord: vec3<f32>,
}

@group(0) @binding(0) var<uniform> d: BasicUniform;

@group(1) @binding(0) var<uniform> circle: CircleData;

fn distance(p1: vec2<f32>, p2: vec2<f32>)-> f32 {
    return sqrt(pow(p1.x - p2.x, 2.0) + pow((p1.y - p2.y) / d.aspect_ratio, 2.0));
}



struct VertexOutput {
    @location(0) color: vec4<f32>,
    @location(1) coord: vec3<f32>,
}

@fragment
fn circle_fs(
    in: VertexOutput
)-> @location(0) vec4<f32> {
    let c = d.camera_coord * d.scale;
    let center = vec2<f32>(circle.center.x + c.x, (circle.center.y - c.y) * d.aspect_ratio) * d.scale;
    if distance(center, in.coord.xy) <= circle.radius * d.scale {
        return in.color;
    } else {
        return vec4<f32>(1.0, 1.0, 1.0, 0.0);
    }
}
