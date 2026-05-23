---
name: iced-popup-window
description: Create separate popup windows with iced UI in winit-based Rust applications (e.g., progress dialogs, modal windows)
license: MIT
compatibility: opencode
metadata:
  audience: developers
  language: rust
---

## What I do

This skill provides patterns for creating separate popup windows with iced UI in a winit-based Rust application. It covers:

1. **Progress Window** - For showing operation progress (e.g., MIDI loading)
2. **Dialog Window** - For modal-like dialogs with custom UI

## When to use me

Use this when you need to:
- Show progress for long-running operations (MIDI loading, file conversion)
- Create modal dialogs that require user input
- Spawn separate windows with their own iced UI rendering

## Key Patterns

### 1. Progress Window Pattern

```rust
// src/runner/progress_manager.rs

use std::sync::Arc;
use tokio::sync::mpsc;
use winit::{dpi, event::WindowEvent, window::WindowAttributes};

pub struct ProgressManager {
    rx: mpsc::UnboundedReceiver<(String, f64)>,
    progress: Option<(String, f64)>,
    window: Option<Arc<winit::window::Window>>,
    gfx: Option<lumino_gfx::Context>,
    ui: Option<lumino_ui::Host>,
}

impl ProgressManager {
    pub fn new() -> (Self, mpsc::UnboundedSender<(String, f64)>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { rx, progress: None, window: None, gfx: None, ui: None }, tx)
    }

    fn create_window(
    }

   mut self, event_loop: &winit::event_loop::ActiveEventLoop, ui_config: &config::UiConfig) {
        let attributes = WindowAttributes::default()
            .with_inner_size(dpi::LogicalSize { width: 500.0, height: 200.0 })
            .with_title("Processing...")
            .with_decorations(true)
            .with_visible(true);

        let window = Arc::new(event_loop.create_window(attributes).expect("Failed"));
        let physical_size = window.inner_size();

        let gfx = futures::executor::block_on(lumino_gfx::Context::new(
            window.clone(), physical_size.width, physical_size.height,
        )).expect("Failed");

        let ui = lumino_ui::Host::new(
            window.clone(), physical_size.width, physical_size.height,
            ui_config, &gfx, true, // is_progress = true
        );

        self.window = Some(window);
        self.gfx = Some(gfx);
        self.ui = Some(ui);
    }

    pub fn handle_event(&mut self, event: WindowEvent) {
        let Some(window) = self.window.clone() else { return };
        match event {
            WindowEvent::RedrawRequested => { /* redraw */ }
            WindowEvent::Resized(size) => { /* resize gfx and ui */ }
            WindowEvent::CloseRequested => { self.close(); }
            _ => { if let Some(ref mut ui) = self.ui { ui.handle_events(event, mods); } }
        }
    }

    pub fn process_messages(&mut self, main_ui: &mut lumino_ui::Host, main_window: &winit::window::Window) {
        while let Ok((msg, progress)) = self.rx            if progress >= 1.0 {
                main.try_recv() {
_ui.update_progress(Some((msg, progress)));
                if let Some(ref mut ui) = self.ui { ui.update_progress(Some((msg, progress))); }
                self.progress = None;
            } else {
                self.progress = Some((msg.clone(), progress));
                main_ui.update_progress(Some((msg, progress)));
                if let Some(ref mut ui) = self.ui { ui.update_progress(Some((msg, progress))); }
            }
        }
    }
}
```

### 2. Dialog Window Pattern

```rust
// src/runner/dialog_manager.rs

use std::collections::HashMap;
use std::sync::Arc;
use winit::{event::WindowEvent, event_loop::ActiveEventLoop, window::{Window, WindowId, WindowAttributes}, dpi::LogicalSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogType {
    CustomPrecision,
}

pub struct DialogWindow {
    window: Arc<Window>,
    gfx: Option<lumino_gfx::Context>,
    ui: Option<lumino_ui::Host>,
    dialog_type: DialogType,
    should_close: bool,
}

impl DialogWindow {
    pub fn new(event_loop: &ActiveEventLoop, dialog_type: DialogType, _parent: Option<&Arc<Window>>) -> Result<Self, String> {
        let attributes = WindowAttributes::default()
            .with_inner_size(LogicalSize { width: 360.0, height: 220.0 })
            .with_title(match dialog_type { DialogType::CustomPrecision => "Custom Precision" })
            .with_visible(false)
            .with_decorations(true)
            .with_resizable(false);

        let window = Arc::new(event_loop.create_window(attributes).map_err(|e| e.to_string())?);
        Ok(Self { window, gfx: None, ui: None, dialog_type, should_close: false, result_data: None })
    }

    pub fn initialize(&mut self, ui_config: &config::UiConfig) -> Result<(), String> {
        let physical_size = self.window.inner_size();
        let gfx = futures::executor::block_on(lumino_gfx::Context::new(self.window.clone(), physical_size.width, physical_size.height))
            .map_err(|e| e.to_string())?;
        let mut ui = lumino_ui::Host::new_dialog(self.window.clone(), physical_size.width, physical_size.height, ui_config, &gfx);
        // Initialize dialog-specific UI state
        ui.set_custom_precision_dialog_open(true);
        self.window.set_visible(true);
        self.gfx = Some(gfx);
        self.ui = Some(ui);
        Ok(())
    }
}
```

### 3. Sending Progress Messages

```rust
// In midi loader or menu.rs
use lumino_core::midi::loader;

// Setup progress sender in runner.rs
lumino_core::midi::loader::set_progress_sender(progress_tx);

// In loading code
lumino_core::midi::loader::send_progress_message("Loading MIDI file...", 0.1);
lumino_core::midi::loader::send_progress_message("Processing tracks...", 0.5);
lumino_core::midi::loader::send_progress_message("Complete!", 1.0);
```

### 4. UI Host for Different Window Types

```rust
// crates/ui/src/host.rs

impl Host {
    // Main window or progress window
    pub fn new(window: Arc<winit::window::Window>, width: u32, height: u32, ui_config: &config::UiConfig, gfx: &lumino_gfx::Context, is_progress: bool) -> Self {
        Self {
            root: if is_progress {
                root::Root::new_progress(&ui_config.theme)
            } else {
                root::Root::new(ui_config)
            },
            // ... other fields
        }
    }

    // Dialog window
    pub fn new_dialog(window: Arc<winit::window::Window>, width: u32, height: u32, ui_config: &config::UiConfig, gfx: &lumino_gfx::Context) -> Self {
        Self {
            root: root::Root::new_dialog(&ui_config.theme),
            // ... other fields
        }
    }
}
```

## Key Components

| Component | Purpose |
|-----------|---------|
| `WindowAttributes` | Configure window properties (size, title, decorations) |
| `lumino_gfx::Context` | WGPU graphics context for rendering |
| `lumino_ui::Host` | Iced UI renderer with window-specific Root |
| `mpsc::UnboundedSender` | Channel for sending progress updates |
| `WindowEvent` | Handle resize, redraw, close events per-window |

## Integration with Event Loop

```rust
// In runner.rs main event loop
fn run() {
    event_loop.run(move |event, event_loop| {
        match event {
            Event::WindowEvent { window_id, event } => {
                if progress_manager.is_progress_window(window_id) {
                    progress_manager.handle_event(event);
                } else if dialog_manager.is_dialog_window(window_id) {
                    if let Some(dialog) = dialog_manager.get_dialog_mut(window_id) {
                        dialog.handle_event(event);
                        if let result = dialog.check_result() {
                            // Handle dialog result
                        }
                        if dialog.should_close() {
                            dialog_manager.close_dialog(window_id);
                        }
                    }
                }
            }
            // ...
        }
    });
}
```
