//! Minimal puffin profiler — lumino style.
//!
//! Just inserts `profile_scope!` in the render pipeline (done in renderer.rs).
//! This example enables profiling at runtime and starts a bridge server so
//! `puffin_viewer` can connect and read the flamegraph.
//!
//! Usage:
//!   cargo run --example profile -p nezha-renderer --features profiling
//!
//! Then open puffin_viewer → connect to localhost:8585.

use std::time::Instant;

use nezha_core::{MidiFile, Note, TempoSegment};
use nezha_renderer::{MidiRenderState, RenderMode, RenderStyle, Renderer};
use wgpu::*;

// ── Synthetic stress data ──────────────────────────────────────────────────────

fn generate_stress() -> MidiFile {
    let bpm = 120.0;
    let ticks_per_beat = 960u32;
    let micros_per_quarter = (60_000_000.0 / bpm) as u64;
    let seconds_per_beat = 60.0 / bpm;
    let notes_per_beat = 64u32; // 256th notes
    let note_dur = seconds_per_beat / notes_per_beat as f64;
    let tick_per_note = ticks_per_beat / notes_per_beat;
    let total_beats = (bpm * 60.0 / 60.0) as u32;
    let total_per_key = total_beats * notes_per_beat;

    let mut key_notes: [Vec<Note>; 128] =
        std::array::from_fn(|_| Vec::with_capacity(total_per_key as usize));

    for key in 0..128u8 {
        let notes = &mut key_notes[key as usize];
        for i in 0..total_per_key {
            let start_sec = i as f64 * note_dur;
            notes.push(Note {
                key,
                start: start_sec,
                end: start_sec + note_dur,
                start_tick: i * tick_per_note,
                end_tick: (i + 1) * tick_per_note,
                velocity: 100,
                channel: 0,
                track: (key % 16) as u16,
            });
        }
    }

    MidiFile {
        key_notes,
        duration: total_per_key as f64 * note_dur,
        ticks_per_beat,
        tempo_segments: vec![TempoSegment {
            start_tick: 0,
            start_time: 0.0,
            micros_per_quarter,
        }],
    }
}

// ── WGPU ───────────────────────────────────────────────────────────────────────

fn setup_wgpu() -> (Instance, Adapter, Device, Queue) {
    let instance = Instance::new(InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("no adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor::default()))
        .expect("no device");
    (instance, adapter, device, queue)
}

fn create_target(device: &Device, w: u32, h: u32) -> (Texture, TextureView) {
    let tex = device.create_texture(&TextureDescriptor {
        label: Some("target"),
        size: Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Bgra8Unorm,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&TextureViewDescriptor::default());
    (tex, view)
}

// ── Main ───────────────────────────────────────────────────────────────────────

fn main() {
    // Enable profiling
    puffin::set_scopes_on(true);

    // Start TCP bridge for puffin_viewer (not a browser!)
    // If port taken, kill stale process:  lsof -ti:8585 | xargs kill -9
    let _server = puffin_http::Server::new("0.0.0.0:8585").expect("puffin_http server");
    println!("🔥 Puffin bridge on localhost:8585");
    println!("   Terminal 2: cargo install puffin_viewer");
    println!("   Terminal 2: puffin_viewer --url 127.0.0.1:8585");
    println!();

    // Setup renderer
    let (_inst, _adapter, device, queue) = setup_wgpu();
    let mut renderer = Renderer::new(device, queue, TextureFormat::Bgra8Unorm);

    let width = 1920u32;
    let height = 1080u32;
    let (_tex, target_view) = create_target(&renderer.device, width, height);

    let midi = generate_stress();
    let note_count: usize = midi.key_notes.iter().map(|v| v.len()).sum();
    println!("📊 {} notes, {:.1}s", note_count, midi.duration);

    renderer.upload_note_data(0, &midi, width, true);

    let style = RenderStyle {
        render_mode: RenderMode::TimeBased,
        border_width: 0.1,
        rounding: 0.0,
        track_index: 0,
        palette: nezha_renderer::random_palette(),
        background: [0.0, 0.0, 0.0, 1.0],
        equal_key_width: true,
        keyboard_height: 100.0,
    };

    let mut state = MidiRenderState::default();
    let frames = 500u32;
    let step = midi.duration / frames as f64;
    let start = Instant::now();

    println!("⏱️  Running {} frames (speed=0.1, max density)...", frames);

    for i in 0..frames {
        puffin::GlobalProfiler::lock().new_frame();

        let t = i as f64 * step;
        let mut encoder = renderer
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });

        renderer.render(
            &mut encoder,
            &target_view,
            width,
            height,
            t,
            0.1f32,
            Some(&midi),
            &mut state,
            Some(0),
            &style,
            true,
        );

        renderer.queue.submit(std::iter::once(encoder.finish()));
        let _ = renderer.device.poll(PollType::Poll);

        if i > 0 && i % 100 == 0 {
            let fps = i as f64 / start.elapsed().as_secs_f64();
            println!("   frame {} / {} — {:.1} fps", i, frames, fps);
        }
    }

    let elapsed = start.elapsed();
    println!(
        "✅ Done: {} frames in {:.1}s ({:.1} fps)",
        frames,
        elapsed.as_secs_f64(),
        frames as f64 / elapsed.as_secs_f64()
    );
    println!("🌐 Puffin bridge still running — press Ctrl+C to exit.");
    println!("   (keep it open while inspecting the flamegraph in puffin_viewer)");
    std::thread::park();
}
