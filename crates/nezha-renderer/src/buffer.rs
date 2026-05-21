use wgpu::*;

use crate::constants::{MAX_INSTANCE_COUNT, MIN_INSTANCE_BUFFER_CAPACITY};

/// A slot holding a GPU instance buffer with known capacity.
pub(crate) struct InstanceBufferSlot {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) capacity_instances: usize,
}

/// Create a new instance buffer slot with the given capacity.
pub(crate) fn create_instance_buffer_slot(
    device: &Device,
    instance_size: u64,
    capacity_instances: usize,
) -> InstanceBufferSlot {
    InstanceBufferSlot {
        buffer: device.create_buffer(&BufferDescriptor {
            label: Some("instance_buffer"),
            size: capacity_instances as u64 * instance_size,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }),
        capacity_instances,
    }
}

/// Compute the next power-of-two capacity for an instance buffer.
pub(crate) fn next_instance_capacity(required_instances: usize) -> usize {
    required_instances
        .max(MIN_INSTANCE_BUFFER_CAPACITY)
        .next_power_of_two()
        .min(MAX_INSTANCE_COUNT)
}
