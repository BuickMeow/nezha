# Nezha — GPU-Accelerated MIDI Visualizer

Nezha（哪吒）is a cross-platform, GPU-accelerated MIDI visualizer built with **Rust**, **wgpu**, and **egui**. It renders waterfall / piano-roll style animations in real time, handling millions of notes with smooth performance.

![Rust](https://img.shields.io/badge/rust-2024%20edition-orange?logo=rust)
![wgpu](https://img.shields.io/badge/wgpu-29-blue)

---

## Features

- **High-performance rendering** — Instanced drawing via wgpu; CPU-parallel instance building with Rayon.
- **Massive MIDI support** — Tested with 900K+ notes; seek-index accelerated scanning for fast seeking.
- **Dual render modes** — `TimeBased` (wall-clock scrolling) and `TickBased` (MIDI-tick aligned).
- **Rich file support** — `.mid` / `.midi`, `.dms`, and archived bundles (`.zip`, `.7z`, `.tar`, `.tar.gz`, `.tar.xz`).
- **Live keyboard overlay** — Real-time key activation with per-track color palettes.
- **Customizable styling** — Palette, rounding, borders, background, equal vs. proportional key widths.
- **Profiling** — Optional `puffin` integration for frame-level flamegraph analysis.

---

## Quick Start

Requires **Rust 1.85+** and a GPU with Vulkan / Metal / DX12 support.

```bash
# Run the desktop GUI
cargo run -p nezha-egui
```

Then use **File → Open** (or drag-and-drop) to load a MIDI, DMS, or archive file.

---

## Project Structure

```
crates/
├── nezha-core/       # MIDI parsing (midly), tempo mapping, tick/time math
├── nezha-renderer/   # wgpu render pipeline, instance builder, shaders
│   ├── src/
│   │   ├── renderer.rs      # Main render logic & CPU instance building
│   │   ├── pipeline.rs      # wgpu pipeline state & bind groups
│   │   ├── shader.wgsl      # Vertex / fragment shaders (SDF rounded rects)
│   │   ├── keyboard.rs      # CPU key layout & keyboard instance generation
│   │   ├── source.rs        # NoteSource trait (decouples renderer from format)
│   │   ├── state.rs         # Per-render mutable state (scan indices, etc.)
│   │   ├── style.rs         # RenderStyle, RenderMode, palette config
│   │   └── vertex.rs        # NoteInstance, Uniforms, GPU type packing
│   └── build.rs             # Shader validation via naga
├── nezha-egui/       # Desktop application (eframe + egui)
│   └── src/
│       ├── app.rs            # Main app shell & file loading
│       ├── config_panel.rs   # Render settings UI
│       ├── piano_view.rs     # Piano keyboard widget
│       ├── transport/        # Timeline ruler, playhead, tracks, controls
│       └── render_context/   # wgpu surface, MIDI cache, preview target
├── nezha-archive/    # ZIP / 7Z / TAR archive reader with MIDI filtering
└── nezha-dms/        # DMS file parser & SMF converter
```

---

## Architecture Highlights

### Rendering Pipeline

1. **Parse** — `nezha-core` reads SMF or DMS into `MidiFile`, grouping 128 key arrays.
2. **Index** — `Renderer::upload_note_data` builds per-key seek indices (block-prefix max-end) for O(1) skipping.
3. **Build** — Each frame, CPU scans visible notes in parallel (Rayon key-chunk groups), emitting `NoteInstance`s.
4. **Draw** — One instanced draw call per 6M-instance batch; vertex shader expands quads with SDF rounded corners.

### Key Optimizations

| Technique | Impact |
|---|---|
| Per-key seek index (256-note blocks) | Eliminates linear scan on seek / rewind |
| Parallel key-group chunking | Balances Rayon tasks by remaining note weight |
| Dynamic instance buffer slots | Power-of-two growth, reused across frames |
| Cached key layouts | Avoids recomputing white/black key geometry each frame |
| Dirty-flag keyboard | Skips keyboard instance rebuild when paused |

---

## Profiling

Enable the `profiling` feature to stream frame scopes to `puffin_viewer`:

```bash
# Terminal 1: run app with profiler
cargo run -p nezha-egui --features profiling

# Terminal 2: open flamegraph viewer
cargo install puffin_viewer
puffin_viewer --url 127.0.0.1:8585
```

Profile scopes (zero-cost when feature is off):
- `render` — full frame
- `scans` — CPU seek-index update
- `keyboard` — CPU keyboard computation
- `render_pass` — GPU render pass
- `upload_note_data` — initial data upload / index build

---

## Input Formats

| Extension | Description | Crate |
|---|---|---|
| `.mid`, `.midi` | Standard MIDI File | `nezha-core` |
| `.dms` | DMS encapsulated MIDI | `nezha-dms` |
| `.zip` | ZIP archive (random access) | `nezha-archive` |
| `.7z` | 7-Zip archive | `nezha-archive` |
| `.tar`, `.tar.gz`, `.tgz`, `.tar.xz`, `.txz` | TAR archives | `nezha-archive` |

Archives are scanned for `.mid` / `.midi` entries; the GUI presents a picker when multiple files are found.

---

## Roadmap

- [x] Move keyboard computation from GPU compute → CPU (eliminates barrier)
- [x] Keyboard dirty flag (skip when paused)
- [x] Parallel key-group instance building with Rayon
- [x] NoteSeekIndex for fast seeking
- [ ] Instance size reduction: 48 → 32 bytes (packed color + f16 props)
- [ ] Workgroup-local atomic counter (reduce global atomic contention)
- [ ] Async compute overlap (double-buffer instance buffer)
- [ ] Fragment shader LOD (fast path for small notes)

---

## License

TBD
