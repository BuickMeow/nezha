@group(0) @binding(0) var<storage> counter: array<u32, 1>;
@group(0) @binding(1) var<storage, read_write> indirect_draw: array<u32, 4>;

const MAX_INSTANCES: u32 = 2700000u;

@compute
@workgroup_size(1)
fn finalize_counts() {
    indirect_draw[1] = min(counter[0], MAX_INSTANCES);
}
