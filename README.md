# Nezha — MIDI Renderer

GPU-accelerated MIDI visualization (waterfall / piano-roll) built with wgpu + egui.

## Quick Start

```sh
cargo run -p nezha-egui
```

## Benchmark

```sh
# Full suite (needs GPU)
cargo bench -p nezha-renderer

# Heaviest scenarios only
cargo bench -p nezha-renderer -- "speed=0.1"

# Upload benchmark only
cargo bench -p nezha-renderer -- upload

# Fast smoke test
cargo bench -p nezha-renderer -- --quick
```

### Baseline (Apple M-series, 1920×1080, 983K notes)

| Scenario | Frame time | ~FPS |
|---|---|---|
| `speed=0.1, t=0s` (heaviest, ~900K instances) | 8–11 ms | 90–120 |
| `speed=1.0` (medium, ~88K instances) | 1–4 ms | 250+ |
| `speed=10.0` (light, ~9K instances) | 0.5–1 ms | 1000+ |
| Upload 1M notes | ~12 ms | 76 Melem/s |

Results at `target/criterion/report/index.html`.

## Flamegraph Profiling

### Real app (load your MIDI, interact normally)

```sh
# Terminal 1: main app with profiler enabled
cargo run -p nezha-egui --features profiling

# Terminal 2: flamegraph window
cargo install puffin_viewer
puffin_viewer --url 127.0.0.1:8585
```

### Synthetic stress test (headless, 983K notes)

```sh
# Terminal 1
cargo run --example profile -p nezha-renderer --features profiling

# Terminal 2
puffin_viewer --url 127.0.0.1:8585
```

> Port conflict? `lsof -ti:8585 | xargs kill -9`

Profile scopes (defined in `crates/nezha-renderer/src/renderer.rs`):
- `render` — full frame
- `scans` — CPU scan index update
- `keyboard` — CPU keyboard computation
- `compute_pass` — GPU compute shader
- `render_pass` — GPU render pass
- `upload_note_data` — initial data upload

Scopes are zero-cost when `profiling` feature is off.

## Project Structure

```
crates/
├── nezha-core/         # MIDI parsing, NoteSource trait
├── nezha-renderer/     # wgpu render pipeline + compute shader
│   ├── src/
│   │   ├── renderer.rs     # main render logic
│   │   ├── shader.wgsl     # vertex/fragment (SDF rounded rects)
│   │   ├── compute_notes.wgsl  # GPU compute shader
│   │   ├── keyboard.rs     # CPU keyboard layout
│   │   └── types.rs        # Renderer, NoteInstance, GPU types
│   ├── benches/
│   │   └── render_bench.rs # criterion benchmarks
│   └── examples/
│       └── profile.rs      # puffin profiler bridge
└── nezha-egui/         # egui/eframe desktop app
```

## Optimization Roadmap

See benchmark results before/after each change.

- [x] Move keyboard computation from GPU compute → CPU (eliminates compute→render barrier for keyboard)
- [x] Keyboard dirty flag (skip recomputation when paused)
- [ ] Instance size reduction: 48 → 32 bytes (packed color + f16 props)
- [ ] Workgroup-local atomic counter (reduce global atomicAdd contention)
- [ ] Async compute overlap (double-buffer instance buffer)
- [ ] Fragment shader LOD (fast path for small notes)
