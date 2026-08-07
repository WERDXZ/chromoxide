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
chrox --config ~/.config/chrox/config.toml ~/Pictures/wallpaper.jpg
```

- `list` shows templates and palettes
- `show` prints palette metadata and members
- `test` solves palettes and prints colors to stdout
- default mode renders configured templates to their output files

For full cli reference, see `chrox --help`
