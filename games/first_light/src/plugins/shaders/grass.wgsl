// Forward-pass vertex shader for wind-swayed grass.
//
// This is the standard bevy_pbr mesh vertex shader (non-skinned, non-morph
// path) with one addition: each vertex is displaced horizontally by a
// travelling wind wave before being projected to clip space. The displacement
// is weighted by `uv.y` (0 at the blade root, 1 at the tip) so blades bend from
// the ground, and offset per blade by `uv.x` so they don't all move in lockstep.
// The fragment stage is left as StandardMaterial's PBR shader, so the grass gets
// full shadows, atmosphere ambient, and fog for free.

#import bevy_pbr::{
    mesh_functions,
    forward_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
}

// x = strength (metres of tip sway), y = speed multiplier, z = time (seconds).
// Time rides in the material uniform rather than the `globals` binding because
// the prepass view layout doesn't expose `globals`, and both passes must sway
// in lockstep.
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> grass_params: vec4<f32>;

fn grass_wind_offset(world_pos: vec3<f32>, bend: f32, phase_rand: f32) -> vec3<f32> {
    let strength = grass_params.x;
    let t = grass_params.z * grass_params.y;
    // Wind blows roughly along +X with a little +Z; the wave travels along it.
    let wind_dir = vec2<f32>(0.92, 0.39);
    let along = dot(world_pos.xz, wind_dir);
    // A slow swell plus a faster flutter, biased positive so the field mostly
    // leans downwind and gusts ripple through it.
    let gust = sin(t * 1.1 + along * 0.16 + phase_rand * 6.2832) * 0.5
             + sin(t * 2.7 + along * 0.45 + phase_rand * 9.0) * 0.18
             + 0.5;
    let bend2 = bend * bend;
    let amp = strength * bend2;
    let horiz = wind_dir * gust * amp;
    // Bending over slightly lowers the tip.
    return vec3<f32>(horiz.x, -0.25 * gust * amp, horiz.y);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);

#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex.instance_index,
    );
#endif

#ifdef VERTEX_POSITIONS
    var world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0),
    );
#ifdef VERTEX_UVS_A
    let offset = grass_wind_offset(world_position.xyz, vertex.uv.y, vertex.uv.x);
    world_position = vec4<f32>(world_position.xyz + offset, world_position.w);
#endif
    out.world_position = world_position;
    out.position = position_world_to_clip(world_position.xyz);
#endif

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        vertex.instance_index,
    );
#endif
#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(
        vertex.instance_index, world_from_local[3]);
#endif

    return out;
}
