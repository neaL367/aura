# Aura — Agent Instructions and Style Guide

This project is a high-performance, low-overhead Windows 11 Desktop Wallpaper Platform named **Aura**. Any AI assistant working on this repository must adhere to the following rules and specifications.

---

## 1. Platform & Toolchain Constraints

- **Target OS**: Windows 11 only. Win32 APIs, undocumented desktop composition messages (`0x052C`), and `WorkerW` desktop layers are utilized.
- **Rust Toolchain**: Pinned to `1.97.1` (as configured in `rust-toolchain.toml`).
- **Cross-Platform Stubs**: For non-Windows developers or CI systems (like Linux runners), target-gated stubs are provided in `crates/platform-windows/src/lib.rs` and `crates/renderer-vulkan/src/lib.rs`. Do not break these stubs when adding platform-specific features.

---

## 2. Vulkan SDK Constraints

- **Vulkan Version**: Pinned to `1.4.350.0` (matching local installations and GitHub Actions configuration).
- **Vulkan CI Cache**: Caching must be disabled (`cache: false`) in the Vulkan installer step in GitHub Actions to avoid deprecated caching library warnings.

---

## 3. Code Style & Linting Guidelines

- **Formatting**: Always format code using `cargo fmt --all`.
- **Clippy**: Code must have zero clippy warnings. Runs on CI with `cargo clippy --workspace --all-targets -- -D warnings`.
- **Win32 Error Precision**: Use `Error::from_thread()` (`GetLastError()`) strictly for genuine Win32 API failures (`RegisterWindowMessageW == 0`, `RegisterClassExW == 0`, `GetMessageW == -1`). Return descriptive domain errors (`Error::new(...)`) for non-API search misses (e.g. `Progman` lookup).
- **Unused Scaffolding**: Binaries/crates under active development must use `#![allow(dead_code)]` at their crate root to prevent lint failures until features are fully wired up.
- **Imports**: Avoid importing unused traits/modules to keep compile times low.

---

## 4. Architectural Rules

- **Crate Layout**:
  - `crates/core`: Platform-independent domain types (monitors, wallpaper lifecycle, configs).
  - `crates/ipc`: Typed length-prefixed JSON protocol over `\\.\pipe\aura-wallpaperd`.
  - `crates/storage`: TOML configs and scanning database cache.
  - `crates/media`: Static image and GIF decoding (using disposal canvas composition).
  - `crates/platform-windows`: Win32 native window wrappers, WorkerW attach, event pump, and singleton.
  - `crates/renderer-vulkan`: Vulkan pipeline, swapchain, texture upload, and `MonitorRenderer`.
  - `crates/wallpaperd`: Aura background daemon coordinator (owns WorkerW, Vulkan render threads, IPC server).
  - `crates/wallpaper-ui`: `egui`/`eframe`-based Control Panel UI.
  - `tools/workerw-proof`: Phase 0 standalone WorkerW validation tool.

- **Threading Architecture**:
  - `wallpaperd`: Main thread runs Win32 event pump loop (`EventPump`), dedicated Tokio thread runs async `IpcServer`, and per-monitor threads run Vulkan swapchain presentation loops.
  - `wallpaper-ui`: Main thread runs `eframe`/`egui` UI, while a background thread with a Tokio runtime manages connection/reconnection to `IpcClient`.
  - `MonitorRenderer`: Stores `Arc<VulkanContext>` and implements `Drop` for RAII resource teardown safety.
  - `ProcessSingleton`: Implements `Send` only (no `Sync`).
