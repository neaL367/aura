# Aura — Windows 11 Desktop Wallpaper Platform

[![Continuous Integration](https://github.com/neaL367/aura/actions/workflows/ci.yml/badge.svg)](https://github.com/neaL367/aura/actions/workflows/ci.yml)

**Aura** is a high-performance, low-overhead Windows 11 desktop wallpaper platform for displaying static images, animated GIFs, and hardware-accelerated video wallpapers behind desktop icons.

---

## Key Features

- **Native Windows 11 Integration**: Reparents host windows directly into the undocumented `WorkerW` desktop composition layer relative to `SHELLDLL_DefView` Z-order behind icons using Win32 desktop composition messages (`0x052C`). Monitor identification uses hardware-stable `MonitorId` hashing via PnP display device queries (`QueryDisplayConfig`).
- **Explorer Restart & Topology Recovery**: Idempotent re-attachment protocol (`ensure_attached()`) with non-fatal state transitions (`Attached` ⇌ `Detached`) automatically recreates host windows and Vulkan surfaces upon Explorer crashes (`TaskbarCreated` broadcast) or display topology changes (`WM_DISPLAYCHANGE`).
- **Vulkan Rendering Pipeline**: Uses `ash` Vulkan bindings with per-monitor Vulkan surface/swapchain isolation, clear-border `border_sampler` for `Fit` and `Center` fitting modes, dirty-flag idle power saving (0% CPU/GPU at rest), bounded in-flight command resources, persistent mapped memory `StagingAllocator` for texture uploads, and RAII `Drop` resource safety.
- **Low-Overhead Decoders & Media Architecture**:
  - **Static Images**: High-performance single-pass RGBA decoding with max 4K automatic downsampling and immediate uncompressed RAM release.
  - **Animated GIFs**: Streaming step-by-step frame decoding with full GIF disposal method compositing (`RestoreToPrevious` snapshot canvas).
  - **Video**: Windows Media Foundation (`IMFSourceReader`) decoding path with AVCC-to-Annex-B conversion and POC reordering.
- **Wallpaper Library, Live Watcher & Gallery UI**: Persistent library of discovered wallpapers stored in a JSON cache (`library.json`). Race-safe thumbnail generation (`ThumbnailStore`) and atomic file saves (`atomic_file.rs`) protect cache integrity. A debounced filesystem watcher (`LibraryWatcher`) automatically synchronizes live watch targets when scan paths are modified. The `wallpaper-ui` Control Panel displays a scrollable gallery grid (`library_panel/`) with per-card **Apply → Display N** assignment buttons and native folder/file pickers (`rfd`).
- **Process Isolation & Resilient IPC**: Headless daemon (`wallpaperd`) and control panel (`wallpaper-ui`) communicate over Windows Named Pipes (`\\.\pipe\aura-wallpaperd`) using an adjacently-tagged JSON protocol (`serde tag+content`). The IPC server accept loop handles pipe connection races and client disconnects cleanly without dropping daemon loops.

---

## Architecture Overview

```text
wallpaper-ui (GUI Control Panel, egui/eframe)
    │
    │ Named Pipe IPC (\\.\pipe\aura-wallpaperd)
    ▼
wallpaperd (Headless Daemon Coordinator)
    ├── Orchestrator (State machine & IPC handlers: status, assignment, library)
    ├── AssignmentManager (Per-monitor wallpaper assignment state)
    ├── RenderCoordinator (Per-monitor Vulkan render loops: placement, loop_runner)
    ├── PerfMonitor (FPS counters, frame latency & process RAM metrics)
    ├── platform-windows (WorkerW management, monitor_enumerator, mf_video_decoder, Win32 pump)
    ├── storage (ConfigStore, LibraryStore, atomic_file, LibraryScanner, LibraryWatcher)
    ├── media (Static Image, GIF streaming compositing, Media Foundation video)
    └── renderer-vulkan (Vulkan context, MonitorRenderer: frame_pass, resources, RAII Drop)
```

---

## System Requirements

- **Operating System**: Windows 11 (build 22000 or newer)
- **Rust Toolchain**: `rustc 1.97.1` (edition 2024)
- **Graphics & SDK**: Vulkan SDK `1.4.350.0` or compatible Vulkan 1.2+ graphics driver
- **Build Tools**: MSVC C++ Build Tools (Windows SDK)

---

## Workspace Structure

The project is structured as a modular Cargo workspace across 8 crates and 1 tool, organized into cohesive domain submodules following Single Responsibility Principle:

| Crate | Purpose |
| :--- | :--- |
| [`aura-core`](crates/core) | Platform-independent domain model (monitors, wallpaper lifecycle, configs) |
| [`aura-ipc`](crates/ipc) | Length-prefixed JSON serialization protocol over Windows Named Pipes |
| [`aura-storage`](crates/storage) | Persistence layer (`ConfigStore`, `LibraryStore`, `atomic_file`, `LibraryScanner`, `LibraryWatcher`) |
| [`aura-media`](crates/media) | Frame-bounded image/GIF decoders and decoder traits |
| [`aura-platform-windows`](crates/platform-windows) | Win32 HWND wrappers, WorkerW (`discovery`, `attachment`, `manager`), `monitor_enumerator`, `mf_video_decoder`, `power` |
| [`aura-renderer-vulkan`](crates/renderer-vulkan) | Vulkan context, monitor renderers (`frame_pass`, `resources`), swapchains, shaders, RAII Drop |
| [`wallpaperd`](crates/wallpaperd) | Headless daemon orchestrator (`handlers/`), `assignment_manager`, `perf_monitor` & render threads (`placement`, `loop_runner`) |
| [`wallpaper-ui`](crates/wallpaper-ui) | `egui`/`eframe` GUI Control Panel (`library_panel/`) & reconnecting IPC client |
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
3. **IPC Serde Tagging**: The `Response` enum uses adjacently-tagged serde (`tag = "type", content = "data"`). Changing this to internally-tagged (`tag = "type"` only) silently breaks deserialization of newtype variants (`WallpaperList`, `Status`) — the UI will show 0 wallpapers with no visible error.

---

## License

Licensed under the MIT License.
