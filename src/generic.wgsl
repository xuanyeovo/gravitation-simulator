
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) coord: vec3<f32>,
}

struct BasicUniform {
    aspect_ratio: f32,
    scale: f32,
    camera_coord: vec3<f32>,
}

@group(0) @binding(0) var<uniform> d: BasicUniform;

@vertex
fn vs_main(
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>
)-> VertexOutput {
    let c = d.camera_coord * d.scale;
    var out: VertexOutput;
    let final_pos = vec3<f32>(pos.x + c.x, (pos.y - c.y) * d.aspect_ratio, pos.z + c.z) * d.scale;

    out.position = vec4<f32>(final_pos, 1.0);
    out.color = color;
    out.coord = final_pos;

    return out;
}

@fragment
fn fs_main(in: VertexOutput)-> @location(0) vec4<f32> {
    return in.color;
}


