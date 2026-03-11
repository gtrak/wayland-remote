A **remote Wayland compositor** that renders to a Windows desktop is feasible. The architecture is essentially:

```
Linux machine
 ├─ Wayland compositor (custom)
 │   ├─ accepts Wayland clients
 │   ├─ renders surfaces
 │   └─ forwards frame updates + input over network
 │
 └─ applications (GTK, Qt, etc.)
 
Network protocol (custom or waypipe-like)

Windows machine
 └─ viewer
     ├─ receives frames
     ├─ displays them in windows
     └─ sends keyboard/mouse input back
```

Two broad implementation strategies exist.

---

# 1. Waypipe-style protocol proxy (simpler)

Instead of writing a compositor, proxy the **Wayland protocol stream**.

This is how waypipe works.

```
app → wayland protocol → proxy → network → proxy → local compositor
```

But your goal differs: Windows has **no Wayland compositor**. Therefore the Windows side must provide one.

Thus the architecture becomes:

```
Linux
 └─ proxy (captures Wayland protocol)

Windows
 └─ lightweight compositor implementing the protocol
```

The Windows compositor would translate:

```
Wayland surfaces → Win32 windows
Wayland input → Wayland protocol
```

Downside:

* Wayland protocol is extremely verbose
* Requires full protocol support
* Harder than it appears

---

# 2. Remote compositor (recommended)

Run the **real compositor on Linux** and only stream frames.

```
Wayland clients
        │
        ▼
 Linux compositor
        │
  frame encoder
        │
     network
        │
        ▼
 Windows viewer
```

Advantages:

* compositor logic stays on Linux
* Windows side only needs a **viewer**
* easier protocol
* similar to RDP / VNC but Wayland-native

---

# 3. Recommended base: Smithay

Use Smithay.

It already provides:

* Wayland compositor framework
* wlroots-like primitives
* renderer abstraction
* input handling

You only implement:

```
backend:
  - headless renderer
  - frame capture
  - network transport
```

---

# 4. Concrete architecture

## Linux compositor

Components:

```
smithay compositor
 ├─ Wayland socket
 ├─ surface tree
 ├─ renderer
 ├─ headless output
 └─ frame capture
```

Instead of DRM output:

```
HeadlessOutput
    ↓
render to texture
    ↓
readback or dmabuf export
    ↓
encode
```

Encoding options:

| Method  | Notes              |
| ------- | ------------------ |
| raw RGB | easiest            |
| H264    | best performance   |
| AV1     | overkill initially |

For MVP: **raw RGBA over TCP**.

---

## Network protocol (simple)

```
struct FrameHeader {
    u32 width
    u32 height
    u64 timestamp
    u32 size
}
```

Sequence:

```
render frame
send header
send pixels
```

Input events:

```
KeyDown
KeyUp
PointerMove
PointerButton
Scroll
```

---

## Windows viewer

Use Rust + Win32.

Window loop:

```
recv frame
upload to texture
blit to window
```

Options:

| API         | Notes    |
| ----------- | -------- |
| Win32 + GDI | easiest  |
| Direct3D11  | faster   |
| wgpu        | portable |

For MVP:

**GDI StretchDIBits**

---

# 5. SSH integration

The compositor provides:

```
WAYLAND_DISPLAY=/tmp/remote-wayland.sock
```

Remote usage:

```
ssh linuxbox
export WAYLAND_DISPLAY=/tmp/remote-wayland.sock
firefox
```

Apps connect to your compositor.

---

# 6. Step-by-step development plan

## Step 1 — Headless Wayland compositor

Using Smithay:

```
cargo new remote-compositor
```

Test:

```
WAYLAND_DISPLAY=wayland-1 weston-simple-egl
```

Expected:

```
client connects
compositor logs surface creation
```

Manual test:

```
WAYLAND_DEBUG=1 weston-simple-egl
```

---

## Step 2 — Render to offscreen buffer

Add a headless output.

Test:

```
render a test surface
save framebuffer as PNG
```

If PNG shows a window → success.

---

## Step 3 — Frame streaming server

Add TCP server:

```
listener.accept()
spawn client thread
```

Send framebuffer each frame.

Test:

```
nc localhost 9000 > frame.raw
```

Inspect with:

```
ffplay -f rawvideo -pixel_format rgba -video_size 1280x720 frame.raw
```

---

## Step 4 — Windows viewer

Create a Rust app:

```
cargo new viewer
```

Responsibilities:

```
connect TCP
read frames
display window
```

Test:

You should see the Linux window.

---

## Step 5 — Input

Send events back:

```
MouseMove
MouseClick
Keyboard
```

Inject them into Smithay input pipeline.

Test:

```
type in remote terminal
```

---

## Step 6 — Window management

Add:

```
xdg-shell support
```

Each surface becomes a **separate Windows window**.

Mapping:

```
Wayland surface ↔ HWND
```

---

# 7. Performance improvements (later)

Once working:

### dmabuf zero-copy

Send GPU buffers directly.

Supported by Smithay.

---

### video encoding

Use:

* ffmpeg
* NVENC / VAAPI

Reduces bandwidth 100x.

---

### damage tracking

Send only changed regions.

---

# 8. Expected complexity

Approximate effort:

| component          | time     |
| ------------------ | -------- |
| minimal compositor | 1 week   |
| frame streaming    | 2–3 days |
| windows viewer     | 2–3 days |
| input              | 2 days   |
| window mapping     | 1 week   |

Total: **~3–4 weeks** part-time.

---

# 9. Key technical challenge

Wayland surfaces often use **GPU buffers (dmabuf)**.

Handling them requires:

```
dmabuf import
GPU readback
```

Smithay already provides most primitives.

---

# 10. Similar existing systems

Relevant projects:

* waypipe — Wayland protocol proxy
* wayvnc — VNC for Wayland
* weston — reference compositor
* Smithay — compositor toolkit

None directly provide **Linux → Windows Wayland streaming**, which is why the idea is viable.

---

If useful, the next step can be a **minimal Smithay compositor skeleton (~200 lines)** that already accepts Wayland clients and renders headlessly.
