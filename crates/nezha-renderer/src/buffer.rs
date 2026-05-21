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
    // Clamp to MAX first so next_power_of_two() never overflows on huge inputs.
    let clamped = required_instances.min(MAX_INSTANCE_COUNT);
    clamped
        .max(MIN_INSTANCE_BUFFER_CAPACITY)
        .next_power_of_two()
        .min(MAX_INSTANCE_COUNT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_min_capacity() {
        // Zero or small requests get bumped to MIN_INSTANCE_BUFFER_CAPACITY
        let cap = next_instance_capacity(0);
        assert_eq!(cap, MIN_INSTANCE_BUFFER_CAPACITY);

        let cap = next_instance_capacity(1);
        assert_eq!(cap, MIN_INSTANCE_BUFFER_CAPACITY);

        let cap = next_instance_capacity(MIN_INSTANCE_BUFFER_CAPACITY - 1);
        assert_eq!(cap, MIN_INSTANCE_BUFFER_CAPACITY);
    }

    #[test]
    fn test_next_power_of_two() {
        let cap = next_instance_capacity(MIN_INSTANCE_BUFFER_CAPACITY + 1);
        assert_eq!(cap, (MIN_INSTANCE_BUFFER_CAPACITY + 1).next_power_of_two());

        let mid = MIN_INSTANCE_BUFFER_CAPACITY * 3;
        let cap = next_instance_capacity(mid);
        assert_eq!(cap, mid.next_power_of_two());
    }

    #[test]
    fn test_cap_at_max() {
        let cap = next_instance_capacity(MAX_INSTANCE_COUNT);
        assert_eq!(cap, MAX_INSTANCE_COUNT);

        let cap = next_instance_capacity(MAX_INSTANCE_COUNT + 1);
        assert_eq!(cap, MAX_INSTANCE_COUNT);

        let cap = next_instance_capacity(usize::MAX);
        assert_eq!(cap, MAX_INSTANCE_COUNT);
    }

    #[test]
    fn test_power_of_two_already() {
        // Exactly power-of-two should stay as-is
        if MIN_INSTANCE_BUFFER_CAPACITY.is_power_of_two() {
            let cap = next_instance_capacity(MIN_INSTANCE_BUFFER_CAPACITY);
            assert_eq!(cap, MIN_INSTANCE_BUFFER_CAPACITY);
        }
    }
}
