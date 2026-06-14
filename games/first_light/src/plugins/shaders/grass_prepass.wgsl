// Prepass (depth/normal/motion) vertex shader for wind-swayed grass.
//
// The camera runs a depth/normal prepass (SSAO and the atmosphere need it), so
// the grass must write the *same* displaced positions here that the forward
// pass writes — otherwise depth wouldn't match and the blades would z-fight or
// vanish. This mirrors bevy_pbr's prepass vertex shader (non-skinned, non-morph)
// and applies the identical wind offset.

#import bevy_pbr::{
    mesh_functions,
    prepass_io::{Vertex, VertexOutput},
    view_transformations::position_world_to_clip,
}

// See grass.wgsl: x = strength, y = speed, z = time (seconds).
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> grass_params: vec4<f32>;

fn grass_wind_offset(world_pos: vec3<f32>, bend: f32, phase_rand: f32) -> vec3<f32> {
    let strength = grass_params.x;
    let t = grass_params.z * grass_params.y;
    let wind_dir = vec2<f32>(0.92, 0.39);
    let along = dot(world_pos.xz, wind_dir);
    let gust = sin(t * 1.1 + along * 0.16 + phase_rand * 6.2832) * 0.5
             + sin(t * 2.7 + along * 0.45 + phase_rand * 9.0) * 0.18
             + 0.5;
    let bend2 = bend * bend;
    let amp = strength * bend2;
    let horiz = wind_dir * gust * amp;
    return vec3<f32>(horiz.x, -0.25 * gust * amp, horiz.y);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
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

#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
#endif

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif

#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex.instance_index,
    );
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(
        world_from_local,
        vertex.tangent,
        vertex.instance_index,
    );
#endif
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

#ifdef MOTION_VECTOR_PREPASS
    let prev_model = mesh_functions::get_previous_world_from_local(vertex.instance_index);
    var prev_world_position = mesh_functions::mesh_position_local_to_world(
        prev_model,
        vec4<f32>(vertex.position, 1.0),
    );
#ifdef VERTEX_UVS_A
    let prev_offset = grass_wind_offset(prev_world_position.xyz, vertex.uv.y, vertex.uv.x);
    prev_world_position = vec4<f32>(prev_world_position.xyz + prev_offset, prev_world_position.w);
#endif
    out.previous_world_position = prev_world_position;
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
