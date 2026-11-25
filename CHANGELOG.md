# Changelog

## 0.5.0

### Hazard3 migration
- Renamed `embassy-st7789v-sprite` to `hazard3-st7789v-sprite`;
  the Rust import is now `hazard3_st7789v_sprite`.
- Replaced the driver with `hazard3-st7789v` 0.8. Re-exported types now
  come from this driver and are not interchangeable with those from the
  old `embassy-st7789v` package.
- `time-hazard3` is enabled by default; `time-upstream` allows using
  upstream Embassy when default features are disabled. Exactly one time
  backend must be selected.
- The legacy `embassy-st7789v-sprite` package was yanked from new crates.io
  resolutions without deleting existing releases.

## 0.4.0

### Added
- New `SpriteEngine` methods: `draw_char`, `draw_str`, `draw_u32`, and
  `draw_f32` write directly to the engine's RAM framebuffer, just like
  `draw_sprite`/`draw_pixel`/`fill_rect`.
- A 5×7 bitmap font (73 glyphs, ASCII + Greek/mathematical symbols)
  duplicated locally in this crate, independently of `embassy-st7789v`:
  no changes are required in `embassy-st7789v`.

### Fixed
- Text flicker: previously, text had to be drawn using
  `St7789v::draw_str`/`draw_u32`/`draw_f32` (direct SPI writes outside the
  framebuffer), causing visible desynchronization between text and the
  rest of the scene on each frame. Text now follows the same path as
  sprites: composed in RAM and sent in a single `flush()`.

### Changed
- `embassy-st7789v` remains at `"0.6"` (no changes to that crate were
  required for this release; text rendering was added entirely within
  `embassy-st7789v-sprite`).

## 0.3.0

### Changed
- Updated the `embassy-st7789v` dependency to `0.6`.
- `SpriteEngine::flush()` now uses `St7789v::blit_u16` to send the entire
  framebuffer in a single continuous SPI transfer (opening the window
  once), enabling more efficient DMA transfers if supported by the driver.

### Added
- Documentation in `README.md` explaining the new `flush()` behavior
  and an implementation example.

## 0.2.0

### Added
- Re-exported Greek and mathematical symbols from `embassy-st7789v`
  (`LAMBDA`, `THETA`, `PI`, `DELTA`, `DEGREE`, `PLUS_MINUS`, `TIMES`,
  `DIVIDE`, `SQRT`, `INFINITY`, `APPROX`, `LE`, `GE`) for scientific
  text rendering via `draw_str` / `draw_str_buf`.

## 0.1.0
- Initial release: `SpriteEngine`, `PiskelSprite`, RAM framebuffer,
  transparency, and automatic clipping.
