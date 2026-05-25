# `samples/`

Local-only image staging area for development. Image binaries are never
tracked in this repo (see `.gitignore`) — only this README and the
`.gitkeep` placeholder under `bench/` are committed.

## `bench/` — fixed contract for the bench harness

The bench harness reads three files **by exact name** out of this
directory. See `crates/photo-frame-bench/src/fixtures.rs`.

| File           | Camera     | Dimensions  | EXIF orient. |
| -------------- | ---------- | ----------- | :----------: |
| `IMG_3936.JPG` | Nikon Z 5  | 6016×4016   | 1            |
| `IMG_3939.JPG` | Nikon Z 5  | 6016×4016   | 1            |
| `IMG_3940.JPG` | Nikon Z 5  | 6016×4016   | 8 (90° CCW)  |

Missing files are not an error — the harness logs a one-line warning
and falls back to synth-only fixtures. CI runs on synth-only by design;
real-world rows in `BENCHMARKS.md` are reproducible only on a machine
that has these three files in place.

## `scratch/` — your throwaway inputs

Drop whatever you want here for ad-hoc CLI runs:

```sh
just run samples/scratch/myphoto.jpg -o /tmp/out.jpg
```

Outputs (`*_framed.jpg` etc.) belong **outside** `samples/`. Point `-o`
at `/tmp` or another scratch path — never back into this tree.
