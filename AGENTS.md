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
  - `crates/storage`: TOML configs (`aura.toml` via `ConfigStore`), JSON wallpaper library cache (`library.json` via `LibraryStore`), and recursive `LibraryScanner` (multi-format media discovery: `png`, `jpg`, `jpeg`, `bmp`, `gif`, `webp`, `mp4`, `mkv`). Paths for both files are resolved under `%APPDATA%/aura` using `dirs::config_dir()`.
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

---

## 5. IPC Protocol Rules

- **Serde Tagging**: The `Response` enum in `crates/ipc/src/protocol.rs` uses `#[serde(tag = "type", content = "data", rename_all = "snake_case")]` (adjacently-tagged). **Never change this to internally-tagged** (`tag = "type"` only). Serde's internally-tagged representation cannot serialize/deserialize tuple/newtype variants such as `Status(DaemonStatus)` or `WallpaperList(Vec<...>)` — it silently returns `Err` on deserialization, causing the UI to always see 0 wallpapers.
- **Response Variants for Mutations**: IPC handlers that mutate library state (`AddScanPath`, `RemoveScanPath`, `RefreshLibrary`) must return `Response::WallpaperList(...)` directly — not `Response::Ok`. This eliminates a fragile second `ListWallpapers` round-trip and ensures the UI gallery updates atomically in a single IPC exchange.
- **Roundtrip Test Coverage**: All four `Response` variants (`Ok`, `Error`, `Status`, `WallpaperList`) must be covered in `crates/ipc/tests/ipc_roundtrip.rs`. Any new response variant added to the protocol must include a roundtrip serialize/deserialize test.
- **egui Frame Loop Guard**: Never call `ipc_client.send(...)` or `ipc_client.fetch_wallpapers()` unconditionally inside `eframe::App::update()`. The egui render loop runs continuously; unguarded sends flood `tokio::sync::mpsc::unbounded_channel` with thousands of duplicate requests per second, blocking real user-triggered IPC commands.

---

## 6. Storage Rules

- **Atomic Write on Windows**: `std::fs::rename(tmp, dest)` fails with `ERROR_ALREADY_EXISTS` if `dest` already exists on Windows (unlike POSIX `rename`). Always call `let _ = std::fs::remove_file(&dest)` before `std::fs::rename(&tmp, &dest)` in `ConfigStore::save` and `LibraryStore::save`.
- **Native File Pickers**: Use `rfd::FileDialog` for all folder/file selection dialogs. `pick_folder()` opens a native Windows folder-only picker; `pick_files()` with `.add_filter("Media Files", &["png", "jpg", "gif", "webp", "mp4", ...])` opens a native file multi-select dialog. Both are synchronous blocking calls on the egui UI thread — this is acceptable.
- **LibraryScanner File vs Directory**: `LibraryScanner::scan_paths()` handles both directory paths (`is_dir()` → recursive scan) and individual file paths (`is_file()` → direct `inspect_file`). Always route `AddScanPath` requests through `LibraryScanner::scan_paths` after adding the path to `config.library.scan_paths`.

---

## 7. WorkerW & Windows Desktop Composition Rules

- **WorkerW Candidate Filtering**: Top-level `WorkerW` fallback checks must filter candidates by client dimensions (`cw >= 300 && ch >= 300`). Windows 11 shell components (such as XAML Islands or Taskbar utility windows) reuse the `WorkerW` class for small internal windows (`120x0`); selecting these breaks desktop window placement.
- **Never Mutate Shell Geometry**: Never call `SetWindowPos` to force-resize Explorer's `WorkerW` window across process boundaries. Explorer automatically sizes desktop-hosting `WorkerW` windows. Forcibly resizing `WorkerW` causes taskbar stalls, desktop UI corruption, and white screen artifacts upon daemon exit.
- **Reject Raw Desktop Window (`#32769`)**: `GetDesktopWindow()` (`#32769`) must never be accepted as a valid `WorkerW` attach target. DWM does not composite `WS_CHILD` windows reparented into `#32769`, causing host windows to silently render nothing.
- **Process-Wide DPI Awareness**: Always call `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` at the very top of `main()` before monitor enumeration or window creation. This prevents Windows from applying silent DPI virtualization and coordinate scaling to `MoveWindow`/`SetWindowPos` on mixed-DPI multi-monitor setups.
