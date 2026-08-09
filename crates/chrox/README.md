# chrox

`chrox` - palette generation tool.

It sits on top of `chromoxide` and `chromoxide-image` and handles:

- builtin palettes
- user-defined palettes in TOML
- simple template rendering
- image-pipeline + solver config

## Builtin palettes

Current builtins include:

- `ansi-16`
- `ansi-8-derived`
- `ansi-16-light`
- `ansi-8-derived-light`
- `base16`
- `base16-bright`
- `cover-salient`

## Template syntax

Templates use simple slot replacement:

```text
{{palette.member}}
{{palette.member | hex}}
{{palette.member | oklch}}
```

If no filter is given, `hex` is used.

## Config

Example:

```toml
[general]
palettes = ["~/.local/share/chrox/palettes"]

[general.reproducibility]
mode = "content_derived"

[[templates]]
name = "alacritty"
input = "templates/alacritty.toml"
output = ".config/alacritty/colors.toml"

[image.saliency]
method = { LocalContrast = { blur_radius = 3, color_weight = 1.0, luminance_weight = 1.0, global_mix = 0.2, robust_normalize = true } }

[image.sampling]
method = { FarthestPointLab = { count = 24, candidate_stride = 2, saliency_bias = 0.35 } }

[config]
seed_count = 32
```

For a complete example, see `assets/example-config.toml`.

Notes:

- template `input` paths are resolved relative to the config file
- relative template `output` paths are resolved from your home directory
- palette search paths come from `[general].palettes` plus CLI `--palettes`
- the default image config caps the working image longest side at 256 pixels
  and builds a statistical cap from prepared pixels (`CapEstimator::Statistical`
  with `CapSource::PreparedPixels`)
- user palettes can use `cap_policy = "Ignore"`, `"HardIntersect"`, or
  `"SoftPenalty"`; cap surfaces are always prebuilt by the image pipeline

### Reproducibility

The CLI defaults to `content_derived`: identical image bytes, image config,
global solve config, palette definition, and seed mode produce the same image
support and palette result within the same determinism algorithm version. Copying
an image to another path does not change its master seed.

```toml
[general.reproducibility]
mode = "content_derived"

# Or use an explicit configured seed:
# mode = "fixed"
# seed = 42

# Or request a fresh seed on every run:
# mode = "random"
```

CLI precedence is `--seed` over `--randomize` over configuration. Random mode
prints `chrox random seed: <u64>` to stderr so the run can be repeated with
`--seed <u64>`. Content-derived and fixed modes are silent. The image pipeline
and each palette solver use separate domain-derived sub-seeds; palettes never
share one mutable RNG, so solve order and unrelated palettes do not perturb a
result.

The core `chromoxide::solve()` API remains random. Library callers that need the
versioned deterministic contract should use `chromoxide::solve_with_seed()`.
Bitwise OkLCh identity is not promised across algorithm versions; within one
version the target is stable final hex output.

## User palettes

User palettes are TOML files made of `slots`, `terms`, and optional solve config.

```toml
name = "my palette"

[[slots]]
name = "bg"
domain = { lightness = { min = 0.10, max = 0.25 }, chroma = { min = 0.00, max = 0.06 }, hue = "Any", cap_policy = "Ignore", chroma_epsilon = 0.02 }

[[terms]]
weight = 4.0
name = "cover"
term = { Cover = { slots = [0], tau = 0.02, delta = 0.03 } }
```

If `id` is omitted, the palette id is derived from the filename.

## CLI

Useful commands:

```bash
chrox list
chrox show base16
chrox test cover-salient base16 -- ~/Pictures/wallpaper.jpg
chrox --seed 42 test cover-salient base16 -- ~/Pictures/wallpaper.jpg
chrox --randomize test cover-salient -- ~/Pictures/wallpaper.jpg
chrox --config ~/.config/chrox/config.toml ~/Pictures/wallpaper.jpg
```

- `list` shows templates and palettes
- `show` prints palette metadata and members
- `test` solves palettes and prints colors to stdout
- default mode renders configured templates to their output files

For full cli reference, see `chrox --help`
