# `samples/`

Local-only image staging area for development.
Image binaries are never tracked in this repository; see `.gitignore`.
Only this README and the `.gitkeep` placeholder under `bench/` are committed.

## `bench/` — fixed contract for the bench harness

The bench harness reads three files **by exact name** from this directory.
See `crates/photo-frame-bench/src/fixtures.rs`.

| File           | Camera    | Dimensions | EXIF orient. |
| -------------- | --------- | ---------- | :----------: |
| `IMG_3936.JPG` | Nikon Z 5 | 6016×4016  |      1       |
| `IMG_3939.JPG` | Nikon Z 5 | 6016×4016  |      1       |
| `IMG_3940.JPG` | Nikon Z 5 | 6016×4016  | 8 (90° CCW)  |

Missing files are not an error; the harness logs a warning and falls back to synthetic fixtures.
CI uses synthetic fixtures.
The real-world rows in `BENCHMARKS.md` require the three files listed above.

## `scratch/` — your throwaway inputs

Drop whatever you want here for ad-hoc CLI runs:

```sh
just run samples/scratch/myphoto.jpg -o /tmp/out.jpg
```

Outputs such as `*_framed.jpg` belong **outside** `samples/`.
Point `-o` at `/tmp` or another scratch path outside the staging area.
