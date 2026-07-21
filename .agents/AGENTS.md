# Aura — Agent Instructions and Style Guide

This project is a high-performance, low-overhead Windows 11 Desktop Wallpaper Platform named **Aura**. Any AI assistant working on this repository must adhere to the following rules and specifications.

---

## 1. Platform & Toolchain Constraints

- **Target OS**: Windows 11 only. Win32 APIs, undocumented desktop messages (`0x052C`), and `WorkerW` layers are utilized.
- **Rust Toolchain**: Pinned to `1.97.1` (as configured in `rust-toolchain.toml`).
- **Cross-Platform Stubs**: For non-Windows developers or CI systems (like Linux runners), target-gated stubs are provided in `crates/platform-windows/src/lib.rs` and `crates/renderer-vulkan/src/lib.rs`. Do not break these stubs when adding platform-specific features.

---

## 2. Vulkan SDK Constraints

- **Vulkan Version**: Pinned to `1.4.350.0` (matching local installations and GitHub Actions configuration).
- **Vulkan CI Cache**: Caching must be disabled (`cache: false`) in the Vulkan installer step in GitHub Actions to avoid deprecated caching library warnings.

---

## 3. Code Style & Linting Guidelines

- **Formatting**: Always format code using `cargo fmt --all`.
- **Clippy**: Code must have zero clippy warnings. Runs on CI with `#![deny(warnings)]` equivalent.
- **Unused Scaffolding**: Binaries/crates under active development must use `#![allow(dead_code)]` at their crate root to prevent lint failures until features are fully wired up.
- **Imports**: Avoid importing unused traits/modules to keep code compile times low.

---

## 4. Architectural Rules

- **Crate Layout**:
  - `crates/core`: Platform-independent domain types (monitor metadata, configs).
  - `crates/ipc`: Named-pipe communication transport.
  - `crates/storage`: TOML configs and scanning database cache.
  - `crates/media`: Static image and GIF decoding (using disposal canvas composition).
  - `crates/platform-windows`: Win32 native window wrappers and stubs.
  - `crates/renderer-vulkan`: Vulkan pipeline, swapchain, and rendering.
  - `crates/wallpaperd`: Aura background service coordinator daemon.
  - `crates/wallpaper-ui`: `egui`-based Control Panel.
