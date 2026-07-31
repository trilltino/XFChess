# Environment backdrops

Equirectangular HDRI photos used for the swappable 3D board background dome
(see `src/rendering/effects/environment.rs`). All sourced from
[Poly Haven](https://polyhaven.com/hdris), licensed **CC0** (public domain —
free for any use, commercial or otherwise, no attribution required).

| File | Source |
|---|---|
| `brown_photostudio_02_1k.hdr` | https://polyhaven.com/a/brown_photostudio_02 |
| `reading_room_1k.hdr` | https://polyhaven.com/a/reading_room |
| `poly_haven_studio_1k.hdr` | https://polyhaven.com/a/poly_haven_studio |
| `wooden_lounge_1k.hdr` | https://polyhaven.com/a/wooden_lounge |

To add another backdrop: download a `.hdr` file (1k resolution is plenty —
~1.5-2MB) from any Poly Haven HDRI page, drop it in this folder, and add a
`(name, path)` entry to `BACKDROP_PRESETS` in `environment.rs`.
