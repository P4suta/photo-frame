# Photo frame documentation

`photo-frame` is a pre-release photo-framing command-line program.
It also provides a browser frontend built with WebAssembly.

## Command-line use

Install the program from a local checkout.

```sh
cargo install --locked --path crates/photo-frame-cli
```

Inputs can use the Joint Photographic Experts Group image format.
Other inputs include Portable Network Graphics and Tagged Image File Format.
Bitmap and WebP inputs are also supported.
The program writes Joint Photographic Experts Group output.

The optional `heif` feature enables High Efficiency Image File Format decoding.
It requires `libheif` version 1.18 or newer on the host.
Debian provides the library through the `libheif-dev` package.

The default output name is `<input-stem>_framed.jpg` beside the source image.
Pass `-o` to choose an output file.
For batch processing, the option can select an output directory.

```sh
photo-frame input.jpg -o framed.jpg
```

The program provides paper and ink themes.
Caption layouts place text at the edges or center.
Presets cover standard, social-media, and highest-quality output.

## Design and implementation

Read [the processing sequence](../crates/photo-frame/src/pipeline.rs).
[Image decoding](../crates/photo-frame-decode/) also detects formats.
[Frame rendering](../crates/photo-frame-frame/) includes frame geometry.
The workspace builds a command-line program and a browser frontend.
Hosted web deployment remains outside the guide's claims.

Read [the dependency policy](../deny.toml) for default constraints.
[Performance notes](PERFORMANCE.md) cover optional image-format support.
The optional format is High Efficiency Image File Format.

## Development references

The [development guide](DEVELOPMENT.md) covers local setup and formatting.
It also covers linting, tests, and continuous integration commands.
[Performance notes](PERFORMANCE.md) describe performance choices.
[Benchmark history](../BENCHMARKS.md) records measurements.
The [event catalog](EVENTS.md) defines observable events.

Local development uses Docker.
The `just ci` command runs formatting, linting, tests, and frontend builds.

## Bundled font attribution

The bundled font is Geist Sans v1.7.1 from Vercel and basement.studio.
It carries the Open Font License, version 1.1.
[Browse the font directory](../crates/photo-frame-frame/assets/fonts/Geist/).
The font directory holds font files and attribution assets.

Read [the license](../crates/photo-frame-frame/assets/fonts/Geist/LICENSE.txt).
Read [font terms](../crates/photo-frame-frame/assets/fonts/Geist/OFL.txt).
Read [authors](../crates/photo-frame-frame/assets/fonts/Geist/AUTHORS.txt).
See [credits](../crates/photo-frame-frame/assets/fonts/Geist/CONTRIBUTORS.txt).

## License

The code's grant remains in [the first license](../LICENSE-MIT).
It also remains in [the second license](../LICENSE-APACHE).
