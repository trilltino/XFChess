---
name: egui
description: Implement XFChess immediate-mode UI with egui and bevy_egui, including input capture, modal state, persistent widget IDs and frame performance.
metadata:
  provenance: docs-derived
---

# egui in Bevy

Read `src/ui/`, `src/ui/egui_ext.rs`, and the UI plugin/schedules. The workspace
pins egui 0.36 and bevy_egui 0.43.1; check the matching crate sources for signatures.

- Keep model state in existing resources. egui reconstructs widgets each frame;
  allocate stable IDs for lists, modals and stateful widgets.
- Respect bevy_egui's context and pass schedules. Do not create a second context
  or mix incompatible egui versions through a new dependency.
- Observe pointer and keyboard capture before processing board picking, shortcuts
  or camera input. Test clicks over windows and modal backgrounds.
- Start network, wallet and engine work from explicit actions; keep the frame path
  free of blocking I/O. Poll owned task results and discard stale completions.
- Reuse fonts, textures and expensive derived data. Release texture/task ownership
  with the owning screen; do not rebuild or spawn them every frame.
- Keep DPI scaling, keyboard navigation and readable contrast intact. Confirm
  layout on the smallest supported window and on the game's actual render target.

Sources: [egui](https://docs.rs/egui/latest/egui/),
[bevy_egui](https://docs.rs/bevy_egui/latest/bevy_egui/),
[integration source](https://github.com/vladbat00/bevy_egui).
