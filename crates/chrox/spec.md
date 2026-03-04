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

4. CLI
  - `chrox IMG [--config path] [--palettes path] [--dry-run]`
  - `chrox list`
  - `chrox show [palette]`

5. Config
  - general entries:
    ...
  - templates entries
  ```toml
    [[template]]
    name = "..."
    input = "some/file"
    output = "some/file"
  ```

6. palette entries:
    ```toml
    name = "..." # id is slugified file name

    [[slots]]
    name = "..."
    other slot constraints in chromoxide

    [[terms]]
    terms in chromoxide
    ```
