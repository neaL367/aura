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
- **Orchestrator Channel Assignment Fallback**: `Request::AssignWallpaper` must not return `unknown monitor` error if `monitor_id` is present in active `state.monitors`. If the render channel (`wallpaper_txs`) is initializing, `Orchestrator` must persist the assignment to `aura.toml` and return success so the UI gallery stays responsive.

---

## 6. Storage & UI Rules

- **Atomic Write on Windows**: `std::fs::rename(tmp, dest)` fails with `ERROR_ALREADY_EXISTS` if `dest` already exists on Windows (unlike POSIX `rename`). Always call `let _ = std::fs::remove_file(&dest)` before `std::fs::rename(&tmp, &dest)` in `ConfigStore::save` and `LibraryStore::save`.
- **Native File Pickers**: Use `rfd::FileDialog` for all folder/file selection dialogs. `pick_folder()` opens a native Windows folder-only picker; `pick_files()` with `.add_filter("Media Files", &["png", "jpg", "gif", "webp", "mp4", ...])` opens a native file multi-select dialog. Both are synchronous blocking calls on the egui UI thread — this is acceptable.
- **LibraryScanner File vs Directory**: `LibraryScanner::scan_paths()` handles both directory paths (`is_dir()` → recursive scan) and individual file paths (`is_file()` → direct `inspect_file`). Always route `AddScanPath` requests through `LibraryScanner::scan_paths` after adding the path to `config.library.scan_paths`.
- **Windows File URI Scheme for egui_extras**: Local Windows file URIs for `egui_extras::install_image_loaders` MUST use 3 slashes (`file:///C:/path/to/file`). Using 2 slashes (`file://C:/...`) causes `egui` to treat `C:` as a network hostname, failing image loading and displaying red warning icons (`⚠`).

---

## 7. WorkerW & Windows Desktop Composition Rules

- **WorkerW Candidate Filtering**: Top-level `WorkerW` fallback checks must filter candidates by client dimensions (`cw >= 300 && ch >= 300`). Windows 11 shell components (such as XAML Islands or Taskbar utility windows) reuse the `WorkerW` class for small internal windows (`120x0`); selecting these breaks desktop window placement.
- **Never Mutate Shell Geometry**: Never call `SetWindowPos` to force-resize Explorer's `WorkerW` window across process boundaries. Explorer automatically sizes desktop-hosting `WorkerW` windows. Forcibly resizing `WorkerW` causes taskbar stalls, desktop UI corruption, and white screen artifacts upon daemon exit.
- **Reject Raw Desktop Window (`#32769`)**: `GetDesktopWindow()` (`#32769`) must never be accepted as a valid `WorkerW` attach target. DWM does not composite `WS_CHILD` windows reparented into `#32769`, causing host windows to silently render nothing.
- **Process-Wide DPI Awareness & Mixed-DPI Hosting**: Always call `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` at the very top of `main()` before monitor enumeration or window creation. Additionally, wrap `SetParent` calls in `attach_to_workerw` with `SetThreadDpiHostingBehavior(DPI_HOSTING_BEHAVIOR_MIXED)` to enable cross-context reparenting into Explorer's `WorkerW` without triggering `ERROR_INVALID_PARAMETER` (`0x80070057`).

---

## 8. Monitor Topology & Reconciliation Rules

- **HMONITOR Handle Invalidation**: Per Microsoft Win32 API specifications, `HMONITOR` handles are transient and become invalid upon `WM_DISPLAYCHANGE`. Monitor identity and active context mapping must rely strictly on stable `MonitorId` values (derived from device paths), never raw `HMONITOR` handles.
- **Dynamic Monitor Reconciliation**: When `HostEvent::DisplayChanged` (`WM_DISPLAYCHANGE`) is received, re-enumerate displays fresh using `MonitorEnumerator::enumerate()`, diff against active `MonitorId`s in `RenderCoordinator`, gracefully shut down removed monitor render threads, spawn new render threads for added displays, and update IPC `Orchestrator` summaries.

---

## 9. Vulkan Teardown & Idempotent Resource Destruction Rules

- **Idempotent Destruction Methods**: All Vulkan resource teardown methods (`Swapchain::destroy`, `GraphicsPipeline::destroy`, `Surface::drop`) MUST clear destroyed handles to `vk::...::null()` immediately after freeing. Calling `destroy()` multiple times on the same object must always be a safe no-op.
- **RAII Resource Teardown & Struct Drop Order**: `MonitorRenderer` implements `Drop` for RAII resource cleanup. Struct fields in `MonitorRenderer` drop in top-to-bottom declaration order; `context: Arc<VulkanContext>` MUST be declared last so Vulkan resources (`Surface`, `Swapchain`, `GraphicsPipeline`) call destroy methods before `VulkanContext` drops. In `MonitorContext`, `render_thread` MUST be declared before `host_window` so render threads join before `HostWindow::drop` destroys the Win32 `HWND`.
- **Vulkan Allocator Teardown Order**: In `VulkanContext::drop`, `allocator: Mutex<Option<gpu_allocator::vulkan::Allocator>>` MUST be explicitly cleared (`lock.take()`) before `device.destroy_device(None)` is invoked, preventing `STATUS_ACCESS_VIOLATION` (`0xc0000005`) crashes on daemon shutdown.

---

## 10. Performance Engine & Memory Optimization Rules

- **Dirty-Flag Presentation Skipping**: Static image presentation loop tracks `is_dirty`. A frame is rendered once on load/resize/fit mode change, after which `renderer.frame(...)` calls are paused to achieve 0% CPU and 0% GPU load during desktop idle.
- **Idle Metric Logging**: `PerfMonitor` logs 0-frame intervals with `status = "Idle (Static - 0% CPU/GPU)"` and `fps = "0.0"` to explicitly distinguish dirty-flag power saving from renderer freezes.
- **Max 4K Static Image Downsampling**: `ImageDecoder::open` MUST automatically downsample static images larger than 4K (3840px) to max 3840px (`img.thumbnail(3840, 3840)`), preventing 6K/8K uncompressed image RAM bloat.
- **Immediate High-Res Image Memory Release**: `ThumbnailStore::get_or_create` MUST explicitly invoke `drop(img)` immediately after `img.thumbnail(...)` generation to release full-resolution 4K/8K uncompressed image RAM before JPEG encoding and file I/O.
- **Heap Vector Compaction**: Scanner results (`LibraryScanner::scan_paths`) and orchestrator library storage (`state.library_items`) MUST invoke `shrink_to_fit()` after scanning to release unused heap allocation capacity.

---

## 11. Media Architecture, Filesystem Watcher & Power Notification Rules

- **Strict Media Crate Platform Independence**: `aura-media` must remain 100% platform-agnostic containing only decoder traits and pure decoders (Image, GIF, WebP). All platform-specific decoders (e.g. Media Foundation `MfVideoDecoder`) MUST reside in `crates/platform-windows/src/mf_video.rs`.
- **Non-Blocking Tokio IPC Operations**: Synchronous file I/O or image decoding inside Tokio IPC handlers MUST be wrapped in `tokio::task::block_in_place(|| ...)` to prevent worker thread starvation.
- **Debounced Filesystem Watcher & Cache Exclusion**: Filesystem watchers MUST use `notify-debouncer-full` with a quiet-period buffer (500ms) to coalesce file event bursts. Events originating within `ThumbnailStore::thumbs_dir()` (`%APPDATA%/aura/thumbs`) MUST be filtered out to prevent self-triggering auto-refresh feedback loops.
- **Win32 Session & Power Notification Lifecycles**: `PowerManager` MUST store its `power_notify_handle: HPOWERNOTIFY` and call `UnregisterPowerSettingNotification` and `WTSUnRegisterSessionNotification` when event pump message windows exit.

---

## 12. Tier 2 Vulkan Video Decoding & Bitstream Rules

- **AVCC to Annex-B Conversion**: Media Foundation NAL samples handing H.264 data MUST pass through `avcc_to_annex_b` to convert 4-byte length prefixes into `0x00000001` start codes for Vulkan Video ingestion.
- **POC Display-Order Reordering**: Decoded H.264 frames MUST pass through `PocReorderBuffer` to sort frames by Picture Order Count (POC) before rendering to prevent scrambled B-frame playback.
- **Dynamic DPB Allocation**: `VulkanVideoSession` MUST dynamically size its Decoded Picture Buffer (DPB) `VkImage` array to `max_num_ref_frames + 1` parsed from the stream's Sequence Parameter Set (SPS).
- **Timeline Semaphore Synchronization**: Queue family ownership transfers between Video Decode Queue (`VIDEO_DECODE_DST`) and Graphics Queue (`SHADER_READ_ONLY`) MUST be synchronized using a Vulkan Timeline Semaphore (`SemaphoreType::TIMELINE`).
- **Session Reset on Loop**: When stream loops or seeks occur, `vkCmdControlVideoCodingKHR` reset flags MUST be submitted to clear reference frame history.



