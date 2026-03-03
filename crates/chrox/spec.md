# Features

1. predefined palettes:
  - ansi colors (16 colors)
  - ansi colors (8 and derive 8 colors)
  - ansi colors bright (16 colors)
  - ansi colors bright (8 and derive 8 colors)
  - base 16
  - base 16 bright
  - 1 salient and 1 cover color

2. user defined palettes
  - some random directory (probably .local/share)
  - toml based? consist of slots and terms

3. templating
  - which syntax??? be simplicit, we just need simple replace feature, and maybe color format conversion
  - Simple syntax for v1: {{palette.name (| filter)?}}
