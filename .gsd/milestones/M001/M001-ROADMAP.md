# M001: Migration

**Vision:** A remote Wayland compositor that runs on Linux and streams application windows to a Windows desktop.

## Success Criteria


## Slices

- [x] **S01: Project Foundation** `risk:medium` `depends:[]`
  > After this: Create Rust virtual workspace root configuration with shared dependencies and toolchain pinning.
- [ ] **S02: Wayland Core Protocol** `risk:medium` `depends:[S01]`
  > After this: Initialize the core Wayland compositor following the Smallvil pattern: set up CompositorState with wayland_frontend feature, create Display integrated with calloop event loop, and accept client connections via ListeningSocketSource.
- [x] **S03: Headless Rendering** `risk:medium` `depends:[S02]`
  > After this: Initialize PixmanRenderer for headless software rendering.
- [x] **S04: Tcp Frame Streaming** `risk:medium` `depends:[S03]`
  > After this: Create streaming module foundation with binary protocol definition and TCP server skeleton.
- [x] **S05: Windows Viewer Foundation** `risk:medium` `depends:[S04]`
  > After this: unit tests prove windows-viewer-foundation works
- [x] **S06: Surface To Hwnd Mapping** `risk:medium` `depends:[S05]`
  > After this: unit tests prove surface-to-hwnd-mapping works
- [ ] **S07: XDG Shell Window Management** `risk:medium` `depends:[S06]`
  > After this: unit tests prove XDG Shell Window Management works
- [ ] **S08: Bidirectional Input** `risk:medium` `depends:[S07]`
  > After this: unit tests prove Bidirectional Input works
