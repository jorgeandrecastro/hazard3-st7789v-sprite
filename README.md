# hazard3-st7789v-sprite

A **Piskel**-style sprite and animation engine with a **RAM framebuffer**
for the **ST7789V** display (240×320). It is part of the independent
[Hazard3 Async](https://github.com/jorgeandrecastro/hazard3-async) ecosystem,
which targets, among others, the RP2350's Hazard3 RISC-V cores. It uses
`hazard3-st7789v` and the `embedded-hal` traits; an upstream Embassy time
backend is also available as an option.

[![Crates.io](https://img.shields.io/crates/v/hazard3-st7789v-sprite.svg)](https://crates.io/crates/hazard3-st7789v-sprite)
[![Docs.rs](https://docs.rs/hazard3-st7789v-sprite/badge.svg)](https://docs.rs/hazard3-st7789v-sprite)
[![License: GPL-2.0-or-later](https://img.shields.io/badge/license-GPL--2.0--or--later-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.html)
![no_std](https://img.shields.io/badge/no__std-yes-brightgreen)

---

## Table of contents

- [How it works](#how-it-works)
- [Installation](#installation)
- [Migration from Embassy](#migration-from-embassy)
- [Quick start](#quick-start)
- [Key concepts](#key-concepts)
  - [The framebuffer](#the-framebuffer)
  - [Piskel sprites](#piskel-sprites)
  - [Transparency](#transparency)
  - [Automatic clipping](#automatic-clipping)
- [API reference](#api-reference)
  - [Constants](#constants)
  - [Greek and mathematical symbols](#greek-and-mathematical-symbols)
  - [`PiskelSprite`](#piskelsprite)
  - [`SpriteEngine`](#spriteengine)
- [Exporting a sprite from Piskel](#exporting-a-sprite-from-piskel)
- [Performance and memory constraints](#performance-and-memory-constraints)
- [License](#license)

---

## How it works

All drawing takes place in a **153,600-byte RAM buffer**
(240 × 320 pixels, each encoded as 16-bit RGB565).

Drawing operations (`clear`, `draw_sprite`, `draw_pixel`, `fill_rect`,
`draw_str`, `draw_u32`, `draw_f32`) are **synchronous** and only modify
this buffer in memory; no SPI communication takes place at this stage.
Once the scene has been composed (background, sprites, HUD, etc.), a
single asynchronous `flush().await` call sends the complete frame to the
physical display over SPI/DMA.

This separation (composing in RAM, then sending the frame as a block)
limits visible updates during composition without guaranteeing tear-free
transfers. **Text now follows the same path** (since 0.4), eliminating
flicker caused by drawing text outside the framebuffer.

```text
┌─────────────┐  clear() / draw_sprite() / draw_str() / fill_rect()  ┌─────────────┐
│   Your      │ ──────────────────────────────────────────────────> │ Framebuffer │
│   game      │                                                     │ RAM         │
│   logic     │ ───────────────── flush().await ───────────────────> │ 153,600 B   │
└─────────────┘               (one SPI transfer)                    └─────────────┘
                                                                           │
                                                                           ▼
                                                                   ST7789V display
```

## Installation

After this version is published on crates.io, add the dependencies to
your `Cargo.toml`:

```toml
[dependencies]
hazard3-st7789v-sprite = "0.5"
hazard3-st7789v        = "0.8"
embedded-hal           = "1.0"
embedded-hal-async     = "1.0"
```

By default, `time-hazard3` selects `hazard3-time` through the driver.
The application must configure a compatible Hazard3 time driver and runtime;
see the [Hazard3 Async quick start](https://github.com/jorgeandrecastro/hazard3-async#quick-start).
Use Hazard3 runtime crates together rather than mixing their time drivers
with upstream Embassy.

To use upstream Embassy instead, disable default features and select
`time-upstream` for both crates:

```toml
[dependencies]
hazard3-st7789v-sprite = { version = "0.5", default-features = false, features = ["time-upstream"] }
hazard3-st7789v = { version = "0.8", default-features = false, features = ["time-upstream"] }
```

This configuration requires an upstream Embassy time driver and runtime.
Exactly one of the `time-hazard3` or `time-upstream` backends must be
enabled. Cargo features are additive across dependencies, so configure all
crates using this driver consistently.

The crate is `#![no_std]` and `#![forbid(unsafe_code)]`: it has no `alloc`
dependency and uses no `unsafe` in the library itself.

## Migration from Embassy

The former `embassy-st7789v-sprite` package was yanked to avoid confusion
with upstream Embassy: existing lockfiles can still resolve it, but new
dependency resolution normally will not select it. Replace it with
`hazard3-st7789v-sprite` and change the `embassy_st7789v_sprite` import to
`hazard3_st7789v_sprite`. Also replace `embassy-st7789v` and its import
with `hazard3-st7789v` and `hazard3_st7789v`: types from these two drivers
are distinct and not interchangeable. The engine API remains unchanged.
These changes do not themselves publish or yank packages on crates.io.

## Quick start

```rust
use embedded_hal::digital::OutputPin;
use embedded_hal_async::spi::SpiDevice;
use hazard3_st7789v_sprite::{Color, PiskelSprite, SpriteEngine, St7789v, FB_SIZE, TRANSPARENT_KEY};

// Sprite exported from Piskel: 4 frames of 16x16 RGB565 pixels.
static HERO_PIXELS: [u16; 16 * 16 * 4] = [ /* ... exported data ... */ TRANSPARENT_KEY; 16 * 16 * 4];

static HERO_SPRITE: PiskelSprite = PiskelSprite {
    width: 16,
    height: 16,
    frame_count: 4,
    pixels: &HERO_PIXELS,
};

async fn draw_frame<SPI: SpiDevice, DC: OutputPin, RST: OutputPin>(
    display: &mut St7789v<SPI, DC, RST>,
    framebuffer: &mut [u16; FB_SIZE],
) -> Result<(), SPI::Error> {
    let mut engine = SpriteEngine::new(display, framebuffer);

    engine.clear(Color(0x0000));                      // 1. black background
    engine.draw_sprite(&HERO_SPRITE, 100, 200, 0);   // 2. hero, frame 0, at (100, 200)
    engine.draw_str(8, 10, b"SCORE: ", Color::WHITE, Color::BLACK); // 3. text in buffer
    engine.draw_u32(62, 10, 42, Color::YELLOW, Color::BLACK);       // 4. value in buffer
    engine.flush().await                              // 5. send the complete frame
}
```

The application supplies the framebuffer and must provide its 153,600 bytes
of RAM, for example in a static allocation with exclusive access.

## Key concepts

### The framebuffer

- Fixed size: `FB_SIZE = SCREEN_W * SCREEN_H = 76,800` pixels, or
  **153,600 bytes** (2 bytes per RGB565 pixel).
- The caller must provide it as `&mut [u16; FB_SIZE]`, placed in a memory
  region suitable for its size and exclusive access.
- `SpriteEngine` never takes ownership: it borrows a mutable reference for
  its lifetime (`'a`).
- **Since 0.4**, text (`draw_str`, `draw_u32`, `draw_f32`) also writes
  to this buffer; no `SpriteEngine` method communicates over SPI except
  `flush()`.

### Piskel sprites

A [`PiskelSprite`](#piskelsprite) represents a sprite sheet exported from
[Piskel](https://www.piskelapp.com/):

- pixels are stored **in flash** (`&'static [u16]`), not RAM;
- all animation frames are concatenated in a single array
  (frame 0, then frame 1, etc.);
- each pixel is encoded as **RGB565**.

### Transparency

The color `TRANSPARENT_KEY` (`0xF81F`, pure magenta) is treated as
transparent by `draw_sprite`: any sprite pixel with this color is **not**
copied into the framebuffer, leaving the previously drawn content
(background, another sprite, etc.) visible underneath.

> ⚠️ Choose this color as the transparent background when exporting from
> Piskel, and avoid using it for an actual sprite detail.

### Automatic clipping

`draw_sprite`, `draw_pixel`, `fill_rect`, `draw_char`, `draw_str`,
`draw_u32`, and `draw_f32` accept signed coordinates (`i16`) and
automatically clip against the display bounds `[0, 240) × [0, 320)`.
A sprite or text can therefore extend partially (or completely) off screen
without panicking or overflowing memory.

## API reference

### Constants

| Constant | Type | Value | Description |
|---|---|---|---|
| `SCREEN_W` | `usize` | `240` | ST7789V display width in pixels. |
| `SCREEN_H` | `usize` | `320` | ST7789V display height in pixels. |
| `FB_SIZE` | `usize` | `76,800` | Total number of framebuffer pixels (`SCREEN_W × SCREEN_H`). |
| `TRANSPARENT_KEY` | `u16` | `0xF81F` | RGB565 color key (magenta) treated as transparent by `draw_sprite`. |

### Greek and mathematical symbols

Since 0.2, this crate has exposed extended codes for scientific text
(λ, θ, π, Δ, °, ±, ×, ÷, √, ∞, ≈, ≤, ≥). **Since 0.4**, these symbols
are directly supported by [`SpriteEngine::draw_str`] and written to the
local framebuffer (a 5×7 font duplicated in this crate, independently of
the one in `hazard3-st7789v`).

These characters are not ASCII: they are represented by bytes in the
`0x80..=0x8C` range, exposed as constants for use in `&[u8]` arrays;
`b'...'` literals cannot represent them.

| Constant | Symbol | Constant | Symbol | Constant | Symbol |
|---|---|---|---|---|---|
| `LAMBDA` | λ | `PLUS_MINUS` | ± | `INFINITY` | ∞ |
| `THETA` | θ | `TIMES` | × | `APPROX` | ≈ |
| `PI` | π | `DIVIDE` | ÷ | `LE` | ≤ |
| `DELTA` | Δ | `SQRT` | √ | `GE` | ≥ |
| `DEGREE` | ° | | | | |

**Example**: display `T=25°C` followed by `R≈3.14×λ` in the framebuffer
before a single `flush()`:

```rust
use hazard3_st7789v_sprite::{Color, DEGREE, APPROX, TIMES, LAMBDA};

// "T=25°C"
engine.draw_str(8, 190, &[b'T', b'=', b'2', b'5', DEGREE, b'C'], Color::YELLOW, Color::BLACK);

// "R≈3.14×λ"
engine.draw_str(8, 210, &[b'R', APPROX, b'3', b'.', b'1', b'4', TIMES, LAMBDA], Color::CYAN, Color::BLACK);
```

> ℹ️ Unlike earlier versions, there is no need to go through
> `engine.driver()` or the `hazard3-st7789v` methods `draw_str`/`draw_str_buf`:
> `SpriteEngine` now handles its own text rendering in the same framebuffer
> as the sprites.

### `PiskelSprite`

A structure representing a static sprite or animation sheet.

```rust
pub struct PiskelSprite {
    pub width: u16,             // frame width in pixels
    pub height: u16,            // frame height in pixels
    pub frame_count: u16,       // total number of frames
    pub pixels: &'static [u16], // RGB565 pixels for all frames, in flash
}
```

Construction example:

```rust
static WALK_PIXELS: [u16; 32 * 32 * 6] = [ /* Piskel export */ ];

static PLAYER_WALK: PiskelSprite = PiskelSprite {
    width: 32,
    height: 32,
    frame_count: 6,
    pixels: &WALK_PIXELS,
};
```

### `SpriteEngine`

```rust
pub struct SpriteEngine<'a, SPI, DC, RST = NoPin>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin;
```

A framebuffer rendering engine for the ST7789V. Generic over the SPI bus,
the DC (data/command) pin, and optionally the RST (reset) pin.

#### Constructors

| Function | Signature | Description |
|---|---|---|
| `SpriteEngine::new` | `fn new(display: &'a mut St7789v<SPI, DC, RST>, framebuffer: &'a mut [u16; FB_SIZE]) -> Self` | Creates an engine for a `St7789v` **with** a hardware RST pin. |
| `SpriteEngine::new_no_rst` | `fn new_no_rst(display: &'a mut St7789v<SPI, DC, NoPin>, framebuffer: &'a mut [u16; FB_SIZE]) -> Self` | Creates an engine for a `St7789v` constructed **without** a hardware RST pin. |

```rust
// With an RST pin
let mut engine = SpriteEngine::new(&mut display, framebuffer);

// Without an RST pin (e.g. RST connected to the MCU's global reset)
let mut engine = SpriteEngine::new_no_rst(&mut display, framebuffer);
```

#### Drawing (synchronous, RAM only)

| Function | Signature | Description |
|---|---|---|
| `clear` | `fn clear(&mut self, color: Color)` | Fills the entire framebuffer with `color`. |
| `draw_sprite` | `fn draw_sprite(&mut self, sprite: &PiskelSprite, start_x: i16, start_y: i16, frame: u16)` | Draws the sprite's `frame` at `(start_x, start_y)`, with automatic transparency and clipping. |
| `draw_pixel` | `fn draw_pixel(&mut self, x: i16, y: i16, color: Color)` | Draws a single pixel (silently ignored if off screen). |
| `fill_rect` | `fn fill_rect(&mut self, x0: i16, y0: i16, x1: i16, y1: i16, color: Color)` | Fills a rectangle `[x0, x1] × [y0, y1]` (inclusive corners), clipped to the display bounds. |

```rust
engine.clear(Color(0x0000));                           // black screen
engine.fill_rect(0, 280, 239, 319, Color(0x0664));    // ground strip at the bottom
engine.draw_pixel(120, 160, Color(0xFFFF));           // white pixel at the center
engine.draw_sprite(&HERO_SPRITE, 100, 200, frame_idx); // hero, current frame
```

#### Text (synchronous, RAM only — new in 0.4)

| Function | Signature | Description |
|---|---|---|
| `draw_char` | `fn draw_char(&mut self, x: i16, y: i16, glyph_idx: usize, fg: Color, bg: Color) -> i16` | Draws a 5×7 glyph by raw index. Returns the x coordinate after the glyph. |
| `draw_str` | `fn draw_str(&mut self, x: i16, y: i16, text: &[u8], fg: Color, bg: Color) -> i16` | Displays an ASCII string (+ extended symbols `0x80..=0x8C`). Returns the x coordinate after the text. |
| `draw_u32` | `fn draw_u32(&mut self, x: i16, y: i16, val: u32, fg: Color, bg: Color) -> i16` | Displays a 32-bit unsigned integer. |
| `draw_f32` | `fn draw_f32(&mut self, x: i16, y: i16, val: f32, decimals: u8, fg: Color, bg: Color) -> i16` | Displays a float with `decimals` digits after the decimal point; handles `NaN`/`+Inf`/`-Inf`. |

These methods use a **locally duplicated 5×7 font** in this crate (the same
glyphs/indices as `hazard3-st7789v`) to write directly into the shared
framebuffer. Otherwise, text would be sent over a separate SPI path from
the rest of the scene, causing visible flicker on each frame.

```rust
engine.draw_str(8, 195, b"LAMAS: ", Color::WHITE, Color::BLACK);
engine.draw_u32(62, 195, 3, Color::YELLOW, Color::BLACK);
let x = engine.draw_f32(8, 210, 23.5, 1, Color::CYAN, Color::BLACK);
engine.draw_str(x, 210, &[hazard3_st7789v_sprite::DEGREE, b'C'], Color::CYAN, Color::BLACK);
```

#### Sending to the display (asynchronous, SPI)

| Function | Signature | Description |
|---|---|---|
| `flush` | `async fn flush(&mut self) -> Result<(), SPI::Error>` | Sends the entire framebuffer to the physical display. |

```rust
engine.flush().await?;
```

The current implementation uses `St7789v::blit_u16` to send the RGB565
buffer in one operation:

```rust
pub async fn flush(&mut self) -> Result<(), SPI::Error> {
    self.display
        .blit_u16(0, 0, (SCREEN_W - 1) as u16, (SCREEN_H - 1) as u16, self.framebuffer)
        .await
}
```

#### Typical animation loop

```rust
loop {
    // 1. Update state BEFORE rendering
    x += 2;
    frame = (frame + 1) % HERO_SPRITE.frame_count;

    // 2. Compose the complete frame in the framebuffer, then flush once
    engine.clear(Color(0x867D));                            // background
    engine.fill_rect(0, 280, 239, 319, Color(0x0664));    // static scenery
    engine.draw_sprite(&HERO_SPRITE, x, 264, frame);       // animated sprite
    engine.draw_str(8, 10, b"SCORE: ", Color::WHITE, Color::BLACK);
    engine.draw_u32(62, 10, score, Color::YELLOW, Color::BLACK);

    engine.flush().await.unwrap();                         // send to display

    Timer::after(Duration::from_millis(33)).await;         // ~30 FPS
}
```

> ⚠️ Draw text **before** `flush()` so it appears in the transmitted frame;
> drawing after it will wait until the next `flush()`.

## Exporting a sprite from Piskel

1. Draw your sprite or animation in [Piskel](https://www.piskelapp.com/).
2. Use **`0xF81F`** (pure magenta, `TRANSPARENT_KEY`) as the transparent
   background color.
3. Export each frame as RGB565, then concatenate them into a single
   `&'static [u16]` array (frame 0 first, then frame 1, etc.).
4. Construct a `PiskelSprite` with the frame width/height, frame count,
   and a reference to the array.

```rust
// 3 frames of 24x24 pixels
static COIN_PIXELS: [u16; 24 * 24 * 3] = [ /* frame 0, frame 1, frame 2 */ ];

static COIN_SPRITE: PiskelSprite = PiskelSprite {
    width: 24,
    height: 24,
    frame_count: 3,
    pixels: &COIN_PIXELS,
};
```

## Performance and memory constraints

- The framebuffer occupies **153,600 bytes** of RAM; ensure your
  microcontroller has enough memory for the rest of the application.
- `flush()` sends the entire framebuffer in a single continuous SPI
  stream using `St7789v::blit_u16` from `hazard3-st7789v`: the write
  window is opened once and all pixels are transferred as a block,
  enabling much more efficient transfers (DMA is possible depending
  on the driver/platform).
- Text (`draw_str`, `draw_u32`, `draw_f32`) runs in `O(characters ×
  35 pixels)` entirely in RAM, with no SPI cost until the final `flush()`.
- All drawing operations run in `O(affected pixels)` and perform no
  allocation (`#![no_std]`, no `alloc` dependency).

## License

Distributed under **GPL-2.0-or-later**. See the [GPL 2.0 text](https://www.gnu.org/licenses/old-licenses/gpl-2.0.html).

## Copyright (C) 2026 Jorge Andre Castro
