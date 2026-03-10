# Phase 1: Project Foundation - Research

**Researched:** 2025-03-10
**Domain:** Rust workspace, Smithay compositor, async networking, Windows GUI
**Confidence:** HIGH

## Summary

This phase establishes the foundational infrastructure for the Wayland Remote project. The research confirms that a virtual workspace with server and viewer crates is the correct approach. Smithay 0.7.0 provides mature Wayland compositor abstractions, Tokio 1.40+ handles async networking requirements, and winit 0.30.x is suitable for Windows window creation. The architecture requires cross-compilation considerations for building the Windows viewer from Linux CI.

**Primary recommendation:** Use a Rust virtual workspace with workspace.dependencies for shared crates, configure Smithay 0.7.0 with headless rendering features, Tokio 1.40+ with full feature set, and winit 0.30.x with Windows-specific features only for the viewer crate.

## User Constraints

*No phase-specific context file exists. Research based on project-level decisions from STATE.md.*

### Locked Decisions
- **Architecture**: Linux Server (Smithay-based headless compositor) → Windows Viewer (native Win32 application)
- **Protocol**: Custom binary TCP streaming with raw RGBA frames
- **Rendering**: PixmanRenderer for software/offscreen rendering
- **Versions**: Smithay 0.7.0, Tokio 1.40+, winit 0.30.x (as specified in ROADMAP)

### Project-Wide Constraints
- Security via SSH tunnel (no built-in authentication)
- Frame streaming approach (not protocol proxy)
- Rust as implementation language
- No web client (native Windows client only)

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| INFRA-01 | Rust workspace structure | Virtual workspace pattern with workspace.dependencies |
| INFRA-02 | Server crate configuration | Smithay 0.7.0 with headless rendering features |
| INFRA-03 | Viewer crate configuration | winit 0.30.x with Windows platform features |
| INFRA-04 | CI/CD pipeline | GitHub Actions with cross-compilation support |
| INFRA-05 | Dependency management | Tokio 1.40+ for async networking |

## Standard Stack

### Core Dependencies

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| smithay | 0.7.0 | Wayland compositor framework | Mature Rust-native alternative to wlroots; designed for compositor building |
| tokio | 1.40+ | Async runtime | Industry standard for Rust async; used by major projects (Smithay ecosystem) |
| winit | 0.30.x | Window creation (Windows) | Most widely-used Rust windowing library; powers egui, Bevy, and others |
| calloop | *via smithay* | Event loop integration | Required by Smithay; callback-based event loop fits compositor model |
| tracing | 0.1.x | Logging | Used extensively by Smithay; structured logging standard |

### Supporting Dependencies

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| wayland-server | *via smithay* | Wayland protocol server | Already included via Smithay reexports |
| wayland-protocols | *via smithay* | Protocol definitions | Use Smithay reexports for version alignment |
| raw-window-handle | 0.6 | Window handle interop | Required for winit integration with graphics APIs |
| smithay-client-toolkit | *optional* | Client-side Wayland | Not needed for server-only crate |
| xkbcommon | *future* | Keyboard handling | Phase 8; defer to input implementation phase |

### Platform-Specific Dependencies

**Server crate (Linux-only):**
```toml
[target.'cfg(unix)'.dependencies]
# Smithay will pull in necessary Linux dependencies
```

**Viewer crate (Windows-only):**
```toml
[target.'cfg(windows)'.dependencies]
# Windows-specific dependencies for GDI/Win32
```

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Smithay | wlroots (via FFI) | Smithay is Rust-native, eliminates FFI complexity, but slightly less mature |
| Tokio | async-std | Tokio is more widely adopted, better ecosystem support, Smithay uses calloop |
| winit | raw Win32 API | winit provides cross-platform abstractions; raw Win32 eliminates dependency but increases code complexity |
| Virtual workspace | Single crate | Workspace enforces clean separation between server/viewer; single crate simpler but mixes concerns |

## Architecture Patterns

### Recommended Workspace Structure

```
wayland-remote/
├── Cargo.toml              # Virtual workspace root
├── Cargo.lock              # Shared lockfile
├── .github/
│   └── workflows/
│       └── ci.yml          # CI/CD configuration
├── crates/
│   ├── server/             # Wayland compositor server
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       └── lib.rs
│   └── viewer/             # Windows viewer application
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
└── shared/                 # Shared types/protocol definitions (if needed)
    ├── Cargo.toml
    └── src/
        └── lib.rs
```

### Pattern 1: Virtual Workspace

**What:** Workspace root without a package, member crates in subdirectories

**When to use:** When there's no "primary" package, or when you want clean separation between distinct components (server vs viewer)

**Example:**
```toml
# [PROJECT_DIR]/Cargo.toml
[workspace]
members = ["crates/server", "crates/viewer", "shared"]
resolver = "3"

[workspace.package]
version = "0.1.0"
edition = "2024"
authors = ["Wayland Remote Contributors"]
license = "MIT OR Apache-2.0"
rust-version = "1.70"

[workspace.dependencies]
# Async runtime
tokio = { version = "1.40", features = ["full"] }

# Logging
tracing = { version = "0.1", features = ["max_level_trace", "release_max_level_debug"] }
tracing-subscriber = "0.3"

# Server-specific
smithay = "0.7.0"

# Viewer-specific (Windows only)
winit = { version = "0.30", default-features = false, features = ["rwh_06"] }
raw-window-handle = "0.6"
```

### Pattern 2: Workspace Dependencies Inheritance

**What:** Define dependencies once in workspace root, inherit in member crates

**When to use:** When multiple crates share the same dependency with same version/features

**Example:**
```toml
# crates/server/Cargo.toml
[package]
name = "wayland-remote-server"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]
tokio = { workspace = true }
tracing = { workspace = true }
smithay = { workspace = true }
```

### Anti-Patterns to Avoid

- **Don't:** Mix server and viewer code in a single crate. They have different target platforms and concerns.
- **Don't:** Use git dependencies for Smithay unless tracking a specific bug fix. Prefer crates.io for stability.
- **Don't:** Enable all winit features on the viewer; only enable what's needed (reduces compile time).
- **Don't:** Pin exact versions in member crates when workspace dependencies are used (use `.workspace = true`).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Wayland protocol handling | Custom protocol dispatch | Smithay's delegate macros | Smithay handles protocol validation, object lifecycle, and globals |
| Event loop integration | Custom event loop | calloop (via Smithay) | calloop is battle-tested, integrates with wayland-server, handles epoll/kqueue |
| Window creation (Windows) | Raw Win32 API calls | winit | winit handles DPI scaling, message loop, multiple backends |
| Async runtime | Thread pool | Tokio | Tokio handles scheduling, IO drivers, timer infrastructure |
| Build configuration | shell scripts | Cargo workspace | Cargo provides dependency resolution, incremental builds, caching |

**Key insight:** Building a Wayland compositor from scratch without Smithay requires thousands of lines of protocol boilerplate and careful state management. Smithay abstracts this while allowing custom window management logic.

## Common Pitfalls

### Pitfall 1: Feature Flag Conflicts Between Smithay and Winit

**What goes wrong:** Smithay and winit both depend on wayland-client with different feature flags, causing "duplicate reference" errors

**Why it happens:** Both libraries pull in wayland-sys with different configurations

**How to avoid:** 
- Use Smithay's reexports where possible (`smithay::wayland::*`)
- If both crates are in the same workspace, ensure consistent feature flags through workspace.dependencies
- Enable `wayland-dlopen` feature in winit to avoid symbol conflicts

**Warning signs:** Build errors mentioning "multiple definitions" or "duplicate symbols" in wayland-client

### Pitfall 2: Resolver Version Mismatch

**What goes wrong:** Virtual workspace must specify resolver explicitly; otherwise build fails with "unable to determine resolver"

**Why it happens:** Virtual manifests have no package.edition to infer resolver version from

**How to avoid:** Always set `resolver = "3"` (or current version) in `[workspace]` section

**Warning signs:** Cargo warning about resolver being undetermined

### Pitfall 3: Cross-Compilation Toolchain Setup

**What goes wrong:** Building Windows viewer on Linux CI fails with linker errors

**Why it happens:** Cross-compiling Rust requires the target toolchain and appropriate linker

**How to avoid:**
- Use `cross` tool or GitHub Actions with windows-latest runner
- For Linux-hosted CI, use `x86_64-pc-windows-gnu` target with mingw-w64
- Document required system packages in README

**Warning signs:** "linker 'cc' not found" or "cannot find -luser32" errors

### Pitfall 4: Smithay Version Pinning

**What goes wrong:** Smithay 0.7.0 API changes in patch releases break compilation

**Why it happens:** Smithay is actively developed, APIs evolve

**How to avoid:**
- Pin exact version: `smithay = "=0.7.0"` (note the extra `=`)
- Or use tilde requirement: `smithay = "~0.7.0"` for patch-only updates
- Review changelog before updating

**Warning signs:** Compilation errors in Smithay-related code after `cargo update`

## Code Examples

### Virtual Workspace Root

```toml
# Cargo.toml
[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.package]
version = "0.1.0"
edition = "2024"
authors = ["Wayland Remote Contributors"]
license = "MIT OR Apache-2.0"
rust-version = "1.70"
repository = "https://github.com/yourusername/wayland-remote"

[workspace.dependencies]
# Shared dependencies
tokio = { version = "1.40", features = ["full"] }
tracing = { version = "0.1", features = ["max_level_trace", "release_max_level_debug"] }
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1.0"
thiserror = "1.0"

# Server crate dependencies
smithay = "=0.7.0"

# Viewer crate dependencies (platform-specific)
winit = { version = "0.30", default-features = false, features = ["rwh_06"] }
raw-window-handle = "0.6"

[workspace.lints.rust]
unsafe_code = "deny"

[profile.release]
opt-level = 3
lto = true
strip = true
```

### Server Crate Configuration

```toml
# crates/server/Cargo.toml
[package]
name = "wayland-remote-server"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Headless Wayland compositor server with frame streaming"
rust-version.workspace = true

[[bin]]
name = "wayland-remote-server"
path = "src/main.rs"

[dependencies]
# Workspace dependencies
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
smithay = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }

# Server-specific dependencies (not in workspace)
# Add as needed for Phase 2-4 implementation

[dev-dependencies]
# Testing utilities
```

### Viewer Crate Configuration

```toml
# crates/viewer/Cargo.toml
[package]
name = "wayland-remote-viewer"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Windows viewer for Wayland remote desktop"
rust-version.workspace = true

[[bin]]
name = "wayland-remote-viewer"
path = "src/main.rs"

[dependencies]
# Workspace dependencies
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }

# Windows-only dependencies
[target.'cfg(windows)'.dependencies]
winit = { workspace = true }
raw-window-handle = { workspace = true }

# Windows API bindings will be added in Phase 5

[dev-dependencies]
# Testing utilities
```

### GitHub Actions CI Configuration

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main, master]
  pull_request:
    branches: [main, master]

env:
  CARGO_TERM_COLOR: always

jobs:
  # Build and test server crate on Linux
  server:
    name: Build Server (Linux)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: stable
      
      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libwayland-dev libxkbcommon-dev
      
      - name: Build server
        run: cargo build --package wayland-remote-server
      
      - name: Test server
        run: cargo test --package wayland-remote-server

  # Build viewer crate on Windows (native)
  viewer-windows:
    name: Build Viewer (Windows)
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: stable
      
      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Build viewer
        run: cargo build --package wayland-remote-viewer
      
      - name: Test viewer
        run: cargo test --package wayland-remote-viewer

  # Cross-compilation check (Linux building Windows target)
  cross-compile:
    name: Cross Compile Check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: stable
          target: x86_64-pc-windows-gnu
      
      - name: Install mingw
        run: |
          sudo apt-get update
          sudo apt-get install -y mingw-w64
      
      - name: Check cross-compile
        run: |
          cargo check --package wayland-remote-viewer --target x86_64-pc-windows-gnu

  # Format and lint
  lint:
    name: Lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: stable
          components: rustfmt, clippy
      
      - name: Check formatting
        run: cargo fmt --all -- --check
      
      - name: Run clippy
        run: cargo clippy --all-targets -- -D warnings
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Smithay 0.3-0.6 | Smithay 0.7.0 | 2024 | Major API redesign with better async support |
| wayland-server 0.30.x | Built into Smithay | 2023 | wayland-server now re-exported via Smithay |
| winit 0.28.x | winit 0.30.x | 2024 | Event loop 3.0 redesign, requires trait-based event handling |
| Resolver 2 | Resolver 3 | Rust 1.83+ | Better dependency resolution, faster builds |

**Deprecated/outdated:**
- wayland-server as direct dependency: Use via Smithay reexports instead
- async-std: Tokio is now the clear ecosystem leader
- glutin/glfw: winit is the modern standard for Rust windowing
- `cargo-tarpaulin` for coverage: Use `cargo llvm-cov` instead

## Validation Architecture

*Validation to be determined in Phase 0 or based on project config. This phase focuses on infrastructure, so validation primarily consists of:*

1. **Build verification:** `cargo build --workspace` succeeds
2. **Test verification:** `cargo test --workspace` runs (even if no tests yet)
3. **CI verification:** GitHub Actions workflow runs successfully
4. **Cross-compilation verification:** Can build viewer target from Linux

### Phase 1 Specific Validation

| Requirement | Test Type | Command |
|-------------|-----------|---------|
| Workspace builds | Build | `cargo build --workspace` |
| Dependencies resolve | Build | `cargo check --workspace` |
| Server compiles | Build | `cargo build --package wayland-remote-server` |
| Viewer compiles | Build | `cargo build --package wayland-remote-viewer` |
| No formatting issues | Lint | `cargo fmt --all -- --check` |
| No clippy warnings | Lint | `cargo clippy --all-targets -- -D warnings` |

## Cross-Compilation Considerations

### Target Platforms

| Crate | Primary Target | CI Build Target | Cross-Compile Target |
|-------|----------------|-----------------|---------------------|
| Server | x86_64-unknown-linux-gnu | ✓ | - |
| Viewer | x86_64-pc-windows-msvc | ✓ (windows-latest) | x86_64-pc-windows-gnu |

### Cross-Compilation Strategy

**Option A: Native Windows CI (Recommended)**
- Use `windows-latest` GitHub Actions runner
- No cross-compilation complexity
- Faster builds, native testing
- Viewer builds natively

**Option B: Linux-hosted Cross-Compilation**
- Use `x86_64-pc-windows-gnu` target
- Requires mingw-w64 toolchain
- Useful for release artifact generation
- Server can be built on same runner

### Recommended CI Matrix

```yaml
strategy:
  matrix:
    include:
      # Server - native Linux build
      - os: ubuntu-latest
        target: x86_64-unknown-linux-gnu
        package: wayland-remote-server
      
      # Viewer - native Windows build
      - os: windows-latest
        target: x86_64-pc-windows-msvc
        package: wayland-remote-viewer
      
      # Viewer - cross-compile from Linux (optional)
      - os: ubuntu-latest
        target: x86_64-pc-windows-gnu
        package: wayland-remote-viewer
```

## Open Questions

1. **Shared Protocol Types**
   - What we know: Both server and viewer need shared frame protocol types
   - What's unclear: Whether to create a `shared` crate or duplicate types
   - Recommendation: Start with duplication, refactor to shared crate in Phase 4 when protocol stabilizes

2. **Smithay Feature Selection**
   - What we know: Smithay has many optional features (backend_drm, backend_egl, etc.)
   - What's unclear: Which features are required for headless/Pixman rendering
   - Recommendation: Start with minimal features, add as needed in Phase 3

3. **Windows API Bindings**
   - What we know: Viewer needs Win32 API for GDI
   - What's unclear: Whether to use `windows` crate, `winapi` crate, or raw bindings
   - Recommendation: Use `windows` crate (modern Microsoft-maintained bindings) in Phase 5

4. **Test Infrastructure**
   - What we know: Integration tests need Wayland client and Windows display
   - What's unclear: How to test in CI without display server
   - Recommendation: Use `WAYLAND_DISPLAY` mocking and headless tests; defer integration testing to Phase 4

## Sources

### Primary (HIGH confidence)
- [Cargo Workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html) - Workspace configuration, inheritance, virtual manifests
- [Smithay 0.7.0 Documentation](https://smithay.github.io/smithay/smithay/) - Crate structure, features, event loop integration
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial) - Runtime setup, async patterns, feature flags
- [winit 0.30.x Docs](https://docs.rs/winit/0.30.13/winit/) - Window creation, platform features, MSRV
- [Cargo Dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html) - Version requirements, platform-specific deps
- [winit CI Workflow](https://github.com/rust-windowing/winit/blob/master/.github/workflows/ci.yml) - Multi-platform CI patterns

### Secondary (MEDIUM confidence)
- [Rust Platform Support](https://doc.rust-lang.org/nightly/rustc/platform-support.html) - Target triples, tier guarantees
- Cargo official book examples and community patterns

### Tertiary (LOW confidence)
- GitHub community discussions on Rust workspace best practices
- Cross-compilation tutorials (verify with official sources before using)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All from official documentation and widely-used crates
- Architecture: HIGH - Based on Cargo book workspace patterns
- Pitfalls: HIGH - Based on documented issues in Smithay/winit ecosystems
- CI/CD: MEDIUM - Based on winit CI (authoritative) but specific needs may vary

**Research date:** 2025-03-10
**Valid until:** 2025-06-10 (re-evaluate Smithay version, check for 0.8 release)

---

*Research complete. Ready for planning phase.*
