# Aura — Windows 11 Desktop Wallpaper Platform

[![Continuous Integration](https://github.com/neaL367/aura/actions/workflows/ci.yml/badge.svg)](https://github.com/neaL367/aura/actions/workflows/ci.yml)

**Aura** is a high-performance, low-overhead Windows 11 desktop wallpaper platform for displaying static images, animated GIFs, and hardware-accelerated video wallpapers behind desktop icons.

---

## Key Features

- **Native Windows 11 Integration**: Reparents host windows directly into the undocumented `WorkerW` desktop composition layer behind icons using Win32 desktop composition messages (`0x052C`).
- **Explorer Restart Recovery**: Idempotent re-attachment protocol (`ensure_attached()`) with non-fatal state transitions (`Attached` ⇌ `Detached`) automatically recovers host windows upon Explorer crashes (`TaskbarCreated` broadcast) or display topology changes (`WM_DISPLAYCHANGE`).
- **Vulkan Rendering Pipeline**: Uses `ash` Vulkan bindings with per-monitor Vulkan surface/swapchain isolation, bounded in-flight command resources, persistent mapped memory `StagingAllocator` for texture uploads, and RAII `Drop` resource safety.
- **Low-Overhead Decoders**:
  - **Static Images**: High-performance single-pass RGBA decoding.
  - **Animated GIFs**: Streaming step-by-step frame decoding with full GIF disposal method compositing (`RestoreToPrevious` snapshot canvas).
  - **Video**: Windows Media Foundation (`IMFSourceReader`) decoding path.
- **Process Isolation & IPC**: Headless daemon (`wallpaperd`) and control panel (`wallpaper-ui`) communicate over Windows Named Pipes (`\\.\pipe\aura-wallpaperd`). The UI features an async reconnecting client (`UiIpcClient`), visual connection status indicators, and pause/resume controls.

---

## Architecture Overview

```text
wallpaper-ui (GUI Control Panel, egui/eframe)
    │
    │ Named Pipe IPC (\\.\pipe\aura-wallpaperd)
    ▼
wallpaperd (Headless Daemon Coordinator)
    ├── Orchestrator (State machine, IPC request routing)
    ├── RenderCoordinator (Per-monitor Vulkan render loops & pause control)
    ├── PerfMonitor (FPS counters & frame latency metrics)
    ├── platform-windows (WorkerW attach, Win32 event pump, monitor enum, power)
    ├── media (Static Image, GIF streaming compositing, Media Foundation video)
    └── renderer-vulkan (Vulkan instance/device, surface, swapchain, shaders, RAII Drop)
```

---

## System Requirements

- **Operating System**: Windows 11 (build 22000 or newer)
- **Rust Toolchain**: `rustc 1.97.1` (edition 2024)
- **Graphics & SDK**: Vulkan SDK `1.4.350.0` or compatible Vulkan 1.2+ graphics driver
- **Build Tools**: MSVC C++ Build Tools (Windows SDK)

---

## Workspace Structure

The project is structured as a modular Cargo workspace across 8 crates and 1 tool:

| Crate | Purpose |
| :--- | :--- |
| [`aura-core`](crates/core) | Platform-independent domain model (monitors, wallpaper lifecycle, configs) |
| [`aura-ipc`](crates/ipc) | Length-prefixed JSON serialization protocol over Windows Named Pipes |
| [`aura-storage`](crates/storage) | Persistence layer for TOML app configs and library JSON database |
| [`aura-media`](crates/media) | Frame-bounded image/GIF decoders and Media Foundation stubs |
| [`aura-platform-windows`](crates/platform-windows) | Win32 HWND wrappers, WorkerW attachments, process singleton |
| [`aura-renderer-vulkan`](crates/renderer-vulkan) | Vulkan context, monitor renderers, swapchains, shaders, RAII Drop |
| [`wallpaperd`](crates/wallpaperd) | Headless background daemon orchestrator & IPC server |
| [`wallpaper-ui`](crates/wallpaper-ui) | `egui`/`eframe` GUI Control Panel & reconnecting IPC client |
| [`workerw-proof`](tools/workerw-proof) | Standalone validation tool for WorkerW integration proof |

---

## Building and Running

### Build All Executables
```powershell
cargo build --workspace --release
```

### Run WorkerW Proof Validation Tool
```powershell
cargo run --bin workerw-proof
```

### Run Desktop Daemon
```powershell
cargo run --bin wallpaperd
```

### Run Control Panel UI
```powershell
cargo run --bin wallpaper-ui
```

### Verification & Testing
```powershell
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

---

## Known Limitations

1. **Windows 11 Only**: Uses Win32 desktop composition messages specific to Windows 11 shell architecture (`WorkerW`).
2. **Video Decoder Tier 1**: Current video pipeline performs CPU-visible frame transfers to Vulkan textures; zero-copy D3D11-to-Vulkan interop is planned for Tier 2.

---

## License

Licensed under the MIT License.
