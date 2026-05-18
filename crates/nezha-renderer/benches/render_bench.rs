//! Render pipeline benchmarks.
//!
//! Usage:
//!   cargo bench -p nezha-renderer
//!   cargo bench -p nezha-renderer -- "speed=0.1"   # heaviest only
//!   cargo bench -p nezha-renderer -- upload         # upload only
//!   cargo bench -p nezha-renderer -- gpu_timing     # GPU timing only
//!   cargo bench -p nezha-renderer -- --quick        # fast smoke test
//!
//! Stress parameters:
//!   120 BPM × 128 keys × 256th notes × 1 minute ≈ 983,040 notes
//!   At speed=0.1: ~983K visible instances (worst case)

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use nezha_core::{MidiFile, Note, TempoSegment};
use nezha_renderer::{MidiRenderState, RenderMode, RenderStyle, Renderer};
use wgpu::*;

// ── Synthetic MIDI generators ──────────────────────────────────────────────────

fn generate_stress_256th() -> MidiFile {
    generate_sequential(120.0, 64, 60.0)
}

fn generate_orchestral() -> MidiFile {
    generate_overlapping(120.0, 4, 4, 60.0)
}

fn generate_sequential(bpm: f64, notes_per_beat: u32, duration_secs: f64) -> MidiFile {
    let ticks_per_beat = 960u32;
    let micros_per_quarter = (60_000_000.0 / bpm) as u64;
    let seconds_per_beat = 60.0 / bpm;
    let note_dur = seconds_per_beat / notes_per_beat as f64;
    let tick_per_note = ticks_per_beat / notes_per_beat;
    let total_beats = (bpm * duration_secs / 60.0) as u32;
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

    let duration = total_per_key as f64 * note_dur;
    MidiFile {
        key_notes,
        duration,
        ticks_per_beat,
        tempo_segments: vec![TempoSegment {
            start_tick: 0,
            start_time: 0.0,
            micros_per_quarter,
        }],
    }
}

fn generate_overlapping(
    bpm: f64,
    notes_per_beat: u32,
    overlap_factor: u32,
    duration_secs: f64,
) -> MidiFile {
    let ticks_per_beat = 960u32;
    let micros_per_quarter = (60_000_000.0 / bpm) as u64;
    let seconds_per_beat = 60.0 / bpm;
    let base_dur = seconds_per_beat / notes_per_beat as f64;
    let note_dur = base_dur * overlap_factor as f64;
    let tick_per_note = ticks_per_beat / notes_per_beat;
    let total_beats = (bpm * duration_secs / 60.0) as u32;
    let total_per_key = total_beats * notes_per_beat;

    let mut key_notes: [Vec<Note>; 128] =
        std::array::from_fn(|_| Vec::with_capacity(total_per_key as usize));

    for key in 0..128u8 {
        let notes = &mut key_notes[key as usize];
        for i in 0..total_per_key {
            let start_sec = i as f64 * base_dur;
            notes.push(Note {
                key,
                start: start_sec,
                end: start_sec + note_dur,
                start_tick: i * tick_per_note,
                end_tick: i * tick_per_note + tick_per_note * overlap_factor,
                velocity: 100,
                channel: 0,
                track: (key % 16) as u16,
            });
        }
    }

    let duration = total_per_key as f64 * base_dur;
    MidiFile {
        key_notes,
        duration,
        ticks_per_beat,
        tempo_segments: vec![TempoSegment {
            start_tick: 0,
            start_time: 0.0,
            micros_per_quarter,
        }],
    }
}

// ── WGPU helpers ────────────────────────────────────────────────────────────────

fn setup_wgpu() -> (Instance, Adapter, Device, Queue) {
    let instance = Instance::new(InstanceDescriptor::new_without_display_handle());
    let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("no adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
        required_features: adapter.features() & Features::TIMESTAMP_QUERY,
        ..Default::default()
    }))
    .expect("no device");
    (instance, adapter, device, queue)
}

fn create_target(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("bench_target"),
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Bgra8Unorm,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor::default());
    (texture, view)
}

// ── Benchmarks ──────────────────────────────────────────────────────────────────

fn bench_render_frame(c: &mut Criterion) {
    let (_instance, _adapter, device, queue) = setup_wgpu();
    let mut renderer = Renderer::new(device, queue, TextureFormat::Bgra8Unorm);

    let width = 1920u32;
    let height = 1080u32;
    let (_target_tex, target_view) = create_target(&*renderer.device(), width, height);

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

    let stress_256 = generate_stress_256th();
    let orchestral = generate_orchestral();

    let datasets: Vec<(&str, &MidiFile)> = vec![
        ("256th_seq_1M", &stress_256),
        ("16th_legato_60K", &orchestral),
    ];

    let speeds = [0.1f32, 1.0, 10.0];
    let time_positions = [0.0f64, 15.0, 30.0, 45.0, 59.0];

    let mut group = c.benchmark_group("render_frame");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.sample_size(50);

    for (ds_name, midi) in &datasets {
        let data_id = if *ds_name == "256th_seq_1M" { 0 } else { 1 };
        renderer.upload_note_data(data_id, *midi, width, true);

        let note_count: usize = midi.key_notes.iter().map(|v| v.len()).sum();
        println!(
            "Dataset '{}': {} notes, {:.1}s duration",
            ds_name, note_count, midi.duration
        );

        for speed in &speeds {
            for &t in &time_positions {
                if t >= midi.duration {
                    continue;
                }
                let label = format!("{}/speed={}/t={:.0}s", ds_name, speed, t);
                let mut state = MidiRenderState::default();

                group.bench_with_input(
                    BenchmarkId::new(label, ""),
                    &(t, *speed, data_id),
                    |b, &(time_pos, spd, id)| {
                        b.iter(|| {
                            let mut encoder = renderer
                                .device
                                .create_command_encoder(&CommandEncoderDescriptor { label: None });
                            renderer.render(
                                &mut encoder,
                                &target_view,
                                width,
                                height,
                                time_pos,
                                spd,
                                Some(*midi),
                                &mut state,
                                Some(id),
                                &style,
                                true,
                            );
                            renderer.queue().submit(std::iter::once(encoder.finish()));
                            let _ = renderer.device().poll(PollType::Poll);
                        });
                    },
                );
            }
        }
    }
    group.finish();
}

/// Benchmark upload_note_data (MIDI → GPU buffers).
///
/// NOTE: this allocates GPU buffers internally — it's a one-time cost per dataset.
/// We keep sample count low because each iteration creates new GPU resources.
fn bench_upload(c: &mut Criterion) {
    let (_instance, _adapter, device, queue) = setup_wgpu();
    let mut renderer = Renderer::new(device, queue, TextureFormat::Bgra8Unorm);
    let width = 1920u32;

    let midi = generate_stress_256th();
    let total_notes: usize = midi.key_notes.iter().map(|v| v.len()).sum();

    let mut group = c.benchmark_group("upload");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(10));
    group.throughput(Throughput::Elements(total_notes as u64));

    let mut next_id = 0usize;
    group.bench_function("256th_1M_notes", |b| {
        b.iter(|| {
            let id = next_id;
            next_id = next_id.wrapping_add(1);
            renderer.upload_note_data(id, &midi, width, true);
        });
    });

    group.finish();
}

/// GPU timing via wgpu timestamp queries — measures actual GPU execution time.
fn bench_gpu_timing(_c: &mut Criterion) {
    let (_instance, _adapter, device, queue) = setup_wgpu();
    let mut renderer = Renderer::new(device, queue, TextureFormat::Bgra8Unorm);

    if !renderer.gpu_timing_available() {
        println!("⚠️  TIMESTAMP_QUERY not supported — skipping GPU timing");
        return;
    }

    let width = 1920u32;
    let height = 1080u32;
    let (_target_tex, target_view) = create_target(&*renderer.device(), width, height);

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

    let midi = generate_stress_256th();
    renderer.upload_note_data(0, &midi, width, true);

    // Quick header
    println!();
    println!("╔══════════════════════════════════════════╗");
    println!("║  GPU Timing (compute / render in ms)      ║");
    println!("╠══════════════════════════════════════════╣");

    for (speed, time_pos) in [(0.1f32, 0.0f64), (0.1, 30.0), (1.0, 0.0), (10.0, 59.0)] {
        let mut state = MidiRenderState::default();

        // Warmup
        for _ in 0..3 {
            let mut encoder = renderer
                .device
                .create_command_encoder(&CommandEncoderDescriptor { label: None });
            renderer.render(
                &mut encoder,
                &target_view,
                width,
                height,
                time_pos,
                speed,
                Some(&midi),
                &mut state,
                Some(0),
                &style,
                true,
            );
            renderer.queue().submit(std::iter::once(encoder.finish()));
            let _ = renderer.device().poll(PollType::Poll);
        }

        // Measure
        let mut total_compute = 0.0f64;
        let mut total_render = 0.0f64;
        let n = 10u32;
        for _ in 0..n {
            let mut encoder = renderer
                .device
                .create_command_encoder(&CommandEncoderDescriptor { label: None });
            renderer.render(
                &mut encoder,
                &target_view,
                width,
                height,
                time_pos,
                speed,
                Some(&midi),
                &mut state,
                Some(0),
                &style,
                true,
            );
            renderer.queue().submit(std::iter::once(encoder.finish()));
            let _ = renderer.device().poll(PollType::Poll);
            if let Some((c, r)) = renderer.read_gpu_timings() {
                total_compute += c;
                total_render += r;
            }
        }
        println!(
            "║ speed={:.1} t={:.0}s │ compute={:6.2}ms  render={:6.2}ms ║",
            speed,
            time_pos,
            total_compute / n as f64,
            total_render / n as f64
        );
    }
    println!("╚══════════════════════════════════════════╝");
}

criterion_group!(benches, bench_render_frame, bench_upload, bench_gpu_timing);
criterion_main!(benches);
