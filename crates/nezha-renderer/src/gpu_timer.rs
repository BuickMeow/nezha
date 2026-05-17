use wgpu::*;

pub struct GpuTimer {
    pub supported: bool,
    pub query_set: Option<QuerySet>,
    pub resolve_buffer: Option<Buffer>,
    pub readback_buffer: Option<Buffer>,
    pub timestamp_period: f32,
}

impl GpuTimer {
    pub fn new(device: &Device, queue: &Queue) -> Self {
        let supported = device.features().contains(Features::TIMESTAMP_QUERY);
        let (query_set, resolve_buffer, readback_buffer) = if supported {
            let qs = device.create_query_set(&QuerySetDescriptor {
                label: Some("timestamps"),
                ty: QueryType::Timestamp,
                count: 4,
            });
            let resolve_buf = device.create_buffer(&BufferDescriptor {
                label: Some("timestamp_resolve"),
                size: 4 * std::mem::size_of::<u64>() as u64,
                usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let readback_buf = device.create_buffer(&BufferDescriptor {
                label: Some("timestamp_readback"),
                size: 4 * std::mem::size_of::<u64>() as u64,
                usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            (Some(qs), Some(resolve_buf), Some(readback_buf))
        } else {
            (None, None, None)
        };

        Self {
            supported,
            query_set,
            resolve_buffer,
            readback_buffer,
            timestamp_period: queue.get_timestamp_period(),
        }
    }

    /// Read back GPU timestamps from the previous frame.
    /// Returns `(compute_ms, render_ms)` or `None` if unsupported or timed out.
    pub fn read_timings(&self, device: &Device) -> Option<(f64, f64)> {
        let readback_buf = self.readback_buffer.as_ref()?;
        let slice = readback_buf.slice(..);

        // Map the buffer for reading
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        // Poll in a loop with timeout — Metal backend doesn't always
        // drive the mapping callback via PollType::Wait alone.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut done = false;
        while std::time::Instant::now() < deadline && !done {
            let _ = device.poll(PollType::Poll);
            done = rx.try_recv().is_ok();
            if !done {
                std::thread::yield_now();
            }
        }
        if !done {
            // Timed out — map may still be pending; ignore this frame
            return None;
        }

        let data = slice.get_mapped_range();
        let timestamps: &[u64] = bytemuck::cast_slice(&data);

        if timestamps.len() < 4 {
            return None;
        }

        // Convert timestamp deltas to milliseconds
        let period = self.timestamp_period as f64;
        let ns_to_ms = 1_000_000.0;

        // Guard against timestamp wrapping/ordering issues
        let compute_ns = if timestamps[1] > timestamps[0] {
            (timestamps[1] - timestamps[0]) as f64 * period
        } else {
            0.0
        };
        let render_ns = if timestamps[3] > timestamps[2] {
            (timestamps[3] - timestamps[2]) as f64 * period
        } else {
            0.0
        };

        // Drop the mapped range so buffer can be used again next frame
        drop(data);
        readback_buf.unmap();

        Some((compute_ns / ns_to_ms, render_ns / ns_to_ms))
    }

    pub fn resolve(&self, encoder: &mut CommandEncoder) {
        if let (Some(qs), Some(resolve_buf), Some(_readback_buf)) =
            (&self.query_set, &self.resolve_buffer, &self.readback_buffer)
        {
            encoder.resolve_query_set(qs, 0..4, resolve_buf, 0);
            encoder.copy_buffer_to_buffer(
                resolve_buf,
                0,
                self.readback_buffer.as_ref().unwrap(),
                0,
                4 * std::mem::size_of::<u64>() as u64,
            );
        }
    }
}
