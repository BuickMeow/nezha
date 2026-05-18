pub(super) struct FrameEncoder {
    current: Option<wgpu::CommandEncoder>,
}

impl Default for FrameEncoder {
    fn default() -> Self {
        Self { current: None }
    }
}

impl FrameEncoder {
    pub(super) fn begin(&mut self, device: &wgpu::Device) {
        self.current = Some(device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render_encoder"),
        }));
    }

    pub(super) fn finish(&mut self, queue: &wgpu::Queue) {
        if let Some(encoder) = self.current.take() {
            queue.submit(std::iter::once(encoder.finish()));
        }
    }

    pub(super) fn encoder_mut(&mut self) -> &mut wgpu::CommandEncoder {
        self.current.as_mut().expect("begin_pass not called")
    }
}
