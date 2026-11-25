#![no_std]
#![forbid(unsafe_code)]

//! # hazard3-st7789v-sprite
//!
//! Piskel-style sprite and animation engine with a RAM framebuffer
//! for the 240×320 ST7789V display, part of the independent
//! [Hazard3 Async](https://github.com/jorgeandrecastro/hazard3-async).
//! The default time backend is `hazard3-time`; for upstream Embassy,
//! disable default features and enable `time-upstream`.
//!
//! ```no_run
//! use embedded_hal::digital::OutputPin;
//! use embedded_hal_async::spi::SpiDevice;
//! use hazard3_st7789v::St7789v;
//! use hazard3_st7789v_sprite::{Color, SpriteEngine, FB_SIZE};
//!
//! async fn draw<SPI: SpiDevice, DC: OutputPin, RST: OutputPin>(
//!     display: &mut St7789v<SPI, DC, RST>,
//!     framebuffer: &mut [u16; FB_SIZE],
//! ) -> Result<(), SPI::Error> {
//!     let mut engine = SpriteEngine::new(display, framebuffer);
//!     engine.clear(Color::BLACK);
//!     engine.flush().await
//! }
//! ```
//!
//! ## How it works
//!
//! All drawing takes place in a 153,600-byte RAM buffer
//! (`SCREEN_W` × `SCREEN_H` pixels encoded as RGB565, or 2 bytes per pixel).
//! All drawing operations ([`SpriteEngine::clear`], [`SpriteEngine::draw_sprite`],
//! [`SpriteEngine::draw_pixel`], [`SpriteEngine::fill_rect`], [`SpriteEngine::draw_str`],
//! [`SpriteEngine::draw_u32`], [`SpriteEngine::draw_f32`]) are **synchronous**
//! and only modify the framebuffer in RAM, without SPI communication.
//! A single asynchronous [`SpriteEngine::flush`] call then sends the complete
//! frame to the physical display over SPI/DMA.
//!
//! This separates composing a complete scene (background, sprites, HUD, etc.)
//! in RAM from sending it as a block. It limits visible updates during
//! composition, without guaranteeing tear-free display updates.
//!
//! ## Why use a local font instead of the one in `hazard3-st7789v`?
//!
//! `St7789v::draw_str` (in `hazard3-st7789v`) writes **directly over SPI**,
//! outside any framebuffer. This crate uses its own `[u16; FB_SIZE]` buffer,
//! composed in RAM and sent as a block via [`SpriteEngine::flush`]. Text must
//! therefore be written to *this* buffer to stay synchronized with sprites;
//! otherwise, sending text and sprites over separate SPI paths causes flicker.
//! This is why the crate has a local copy of the 5×7 font.

use embedded_hal::digital::OutputPin;
use embedded_hal_async::spi::SpiDevice;

// Re-export types and symbols useful to the end user.
pub use hazard3_st7789v::{
    Color, NoPin, St7789v, St7789vBuffered,
    APPROX, DEGREE, DELTA, DIVIDE, GE, INFINITY, LAMBDA, LE, PI, PLUS_MINUS, SQRT, THETA, TIMES,
};

/// ST7789V display width in pixels.
pub const SCREEN_W: usize = 240;

/// ST7789V display height in pixels.
pub const SCREEN_H: usize = 320;

/// Total number of framebuffer pixels (SCREEN_W × SCREEN_H).
pub const FB_SIZE: usize = SCREEN_W * SCREEN_H; // 76,800 pixels → 153,600 bytes

/// Transparency color key used by [`SpriteEngine::draw_sprite`].
pub const TRANSPARENT_KEY: u16 = 0xF81F;

// ─────────────────────────────────────────────────────────────────────────────
// 5×7 bitmap font: local copy, independent of hazard3-st7789v.
// The same glyphs and indices as the driver's font keep the appearance consistent.
// ─────────────────────────────────────────────────────────────────────────────

const FONT: [[u8; 5]; 73] = [
    [0x3E, 0x51, 0x49, 0x45, 0x3E], // 0
    [0x00, 0x42, 0x7F, 0x40, 0x00], // 1
    [0x42, 0x61, 0x51, 0x49, 0x46], // 2
    [0x21, 0x41, 0x45, 0x4B, 0x31], // 3
    [0x18, 0x14, 0x12, 0x7F, 0x10], // 4
    [0x27, 0x45, 0x45, 0x45, 0x39], // 5
    [0x3C, 0x4A, 0x49, 0x49, 0x30], // 6
    [0x01, 0x71, 0x09, 0x05, 0x03], // 7
    [0x36, 0x49, 0x49, 0x49, 0x36], // 8
    [0x06, 0x49, 0x49, 0x29, 0x1E], // 9
    [0x08, 0x08, 0x08, 0x08, 0x08], // 10 = '-'
    [0x00, 0x00, 0x00, 0x00, 0x00], // 11 = ' '
    [0x7E, 0x11, 0x11, 0x11, 0x7E], // 12 = 'A'
    [0x7F, 0x49, 0x49, 0x49, 0x36], // 13 = 'B'
    [0x3E, 0x41, 0x41, 0x41, 0x22], // 14 = 'C'
    [0x7F, 0x41, 0x41, 0x22, 0x1C], // 15 = 'D'
    [0x7F, 0x49, 0x49, 0x49, 0x41], // 16 = 'E'
    [0x7F, 0x09, 0x09, 0x09, 0x01], // 17 = 'F'
    [0x3E, 0x41, 0x49, 0x49, 0x7A], // 18 = 'G'
    [0x7F, 0x08, 0x08, 0x08, 0x7F], // 19 = 'H'
    [0x00, 0x41, 0x7F, 0x41, 0x00], // 20 = 'I'
    [0x20, 0x40, 0x41, 0x3F, 0x01], // 21 = 'J'
    [0x7F, 0x08, 0x14, 0x22, 0x41], // 22 = 'K'
    [0x7F, 0x40, 0x40, 0x40, 0x40], // 23 = 'L'
    [0x7F, 0x02, 0x0C, 0x02, 0x7F], // 24 = 'M'
    [0x7F, 0x04, 0x08, 0x10, 0x7F], // 25 = 'N'
    [0x3E, 0x41, 0x41, 0x41, 0x3E], // 26 = 'O'
    [0x7F, 0x09, 0x09, 0x09, 0x06], // 27 = 'P'
    [0x3E, 0x41, 0x51, 0x21, 0x5E], // 28 = 'Q'
    [0x7F, 0x09, 0x19, 0x29, 0x46], // 29 = 'R'
    [0x46, 0x49, 0x49, 0x49, 0x31], // 30 = 'S'
    [0x01, 0x01, 0x7F, 0x01, 0x01], // 31 = 'T'
    [0x3F, 0x40, 0x40, 0x40, 0x3F], // 32 = 'U'
    [0x1F, 0x20, 0x40, 0x20, 0x1F], // 33 = 'V'
    [0x3F, 0x40, 0x38, 0x40, 0x3F], // 34 = 'W'
    [0x63, 0x14, 0x08, 0x14, 0x63], // 35 = 'X'
    [0x07, 0x08, 0x70, 0x08, 0x07], // 36 = 'Y'
    [0x61, 0x51, 0x49, 0x45, 0x43], // 37 = 'Z'
    [0x00, 0x00, 0x60, 0x60, 0x00], // 38 = '.'
    [0x00, 0x3E, 0x41, 0x41, 0x00], // 39 = '('
    [0x00, 0x41, 0x41, 0x3E, 0x00], // 40 = ')'
    [0x00, 0x40, 0x50, 0x30, 0x00], // 41 = ','
    [0x00, 0x7F, 0x41, 0x41, 0x00], // 42 = '['
    [0x00, 0x41, 0x41, 0x7F, 0x00], // 43 = ']'
    [0x23, 0x13, 0x08, 0x64, 0x62], // 44 = '%'
    [0x08, 0x14, 0x22, 0x41, 0x00], // 45 = '<'
    [0x00, 0x41, 0x22, 0x14, 0x08], // 46 = '>'
    [0x00, 0x24, 0x24, 0x24, 0x00], // 47 = '='
    [0x02, 0x01, 0x51, 0x09, 0x06], // 48 = '?'
    [0x00, 0x00, 0x5F, 0x00, 0x00], // 49 = '!'
    [0x00, 0x36, 0x36, 0x00, 0x00], // 50 = ':'
    [0x08, 0x08, 0x3E, 0x08, 0x08], // 51 = '+'
    [0x20, 0x10, 0x08, 0x04, 0x02], // 52 = '/'
    [0x00, 0x00, 0x7F, 0x00, 0x00], // 53 = '|'
    [0x40, 0x40, 0x40, 0x40, 0x40], // 54 = '_'
    [0x04, 0x02, 0x01, 0x02, 0x04], // 55 = '^'
    [0x14, 0x7F, 0x14, 0x7F, 0x14], // 56 = '#'
    [0x3E, 0x41, 0x5D, 0x55, 0x1E], // 57 = '@'
    [0x32, 0x49, 0x55, 0x22, 0x50], // 58 = '&'
    [0x00, 0x07, 0x00, 0x07, 0x00], // 59 = '"'
    [0x60, 0x10, 0x0F, 0x10, 0x60], // 60 = 'λ'
    [0x3E, 0x49, 0x49, 0x49, 0x3E], // 61 = 'θ'
    [0x01, 0x7D, 0x01, 0x7D, 0x01], // 62 = 'π'
    [0x78, 0x46, 0x41, 0x46, 0x78], // 63 = 'Δ'
    [0x06, 0x09, 0x09, 0x06, 0x00], // 64 = '°'
    [0x48, 0x48, 0x5E, 0x48, 0x48], // 65 = '±'
    [0x22, 0x14, 0x08, 0x14, 0x22], // 66 = '×'
    [0x08, 0x08, 0x49, 0x08, 0x08], // 67 = '÷'
    [0x70, 0x4C, 0x03, 0x01, 0x01], // 68 = '√'
    [0x1C, 0x22, 0x1C, 0x22, 0x1C], // 69 = '∞'
    [0x00, 0x24, 0x12, 0x12, 0x24], // 70 = '≈'
    [0x60, 0x68, 0x64, 0x62, 0x61], // 71 = '≤'
    [0x61, 0x62, 0x64, 0x68, 0x60], // 72 = '≥'
];

fn char_to_glyph(c: u8) -> Option<usize> {
    match c {
        b'0'..=b'9' => Some((c - b'0') as usize),
        b'-'        => Some(10),
        b' '        => Some(11),
        b'A'..=b'Z' => Some((c - b'A') as usize + 12),
        b'a'..=b'z' => Some((c - b'a') as usize + 12),
        b'.'        => Some(38),
        b'('        => Some(39),
        b')'        => Some(40),
        b','        => Some(41),
        b'['        => Some(42),
        b']'        => Some(43),
        b'%'        => Some(44),
        b'<'        => Some(45),
        b'>'        => Some(46),
        b'='        => Some(47),
        b'?'        => Some(48),
        b'!'        => Some(49),
        b':'        => Some(50),
        b'+'        => Some(51),
        b'/'        => Some(52),
        b'|'        => Some(53),
        b'_'        => Some(54),
        b'^'        => Some(55),
        b'#'        => Some(56),
        b'@'        => Some(57),
        b'&'        => Some(58),
        b'"'        => Some(59),
        LAMBDA      => Some(60),
        THETA       => Some(61),
        PI          => Some(62),
        DELTA       => Some(63),
        DEGREE      => Some(64),
        PLUS_MINUS  => Some(65),
        TIMES       => Some(66),
        DIVIDE      => Some(67),
        SQRT        => Some(68),
        INFINITY    => Some(69),
        APPROX      => Some(70),
        LE          => Some(71),
        GE          => Some(72),
        _           => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PiskelSprite
// ─────────────────────────────────────────────────────────────────────────────

/// Static sprite or animation sheet exported from Piskel.
///
/// Pixel data is stored in flash (`&'static [u16]`).
/// Each pixel is encoded as 16-bit **RGB565**.
/// Pixels matching [`TRANSPARENT_KEY`] are treated as transparent by
/// [`SpriteEngine::draw_sprite`].
#[derive(Clone, Copy)]
pub struct PiskelSprite {
    /// Width of one frame in pixels.
    pub width: u16,
    /// Height of one frame in pixels.
    pub height: u16,
    /// Total number of frames in the sheet.
    pub frame_count: u16,
    /// RGB565 data for all frames, in flash.
    pub pixels: &'static [u16],
}

// ─────────────────────────────────────────────────────────────────────────────
// SpriteEngine
// ─────────────────────────────────────────────────────────────────────────────

/// Framebuffer-based rendering engine for the ST7789V.
pub struct SpriteEngine<'a, SPI, DC, RST = NoPin>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
{
    display: &'a mut St7789v<SPI, DC, RST>,
    framebuffer: &'a mut [u16; FB_SIZE],
}

// ── Constructor without an RST pin ───────────────────────────────────────────

impl<'a, SPI, DC> SpriteEngine<'a, SPI, DC, NoPin>
where
    SPI: SpiDevice,
    DC: OutputPin,
{
    /// Creates an engine for a `St7789v` constructed **without** a hardware RST pin.
    #[inline]
    pub fn new_no_rst(
        display: &'a mut St7789v<SPI, DC, NoPin>,
        framebuffer: &'a mut [u16; FB_SIZE],
    ) -> Self {
        Self { display, framebuffer }
    }
}

// ── Constructor with an RST pin ──────────────────────────────────────────────

impl<'a, SPI, DC, RST> SpriteEngine<'a, SPI, DC, RST>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
{
    /// Creates an engine for a `St7789v` with a hardware RST pin.
    #[inline]
    pub fn new(
        display: &'a mut St7789v<SPI, DC, RST>,
        framebuffer: &'a mut [u16; FB_SIZE],
    ) -> Self {
        Self { display, framebuffer }
    }

    // ── Framebuffer operations (synchronous, RAM only) ────────────────────────

    /// Fills the **entire** RAM framebuffer with `color` (no SPI communication).
    #[inline]
    pub fn clear(&mut self, color: Color) {
        self.framebuffer.fill(color.0);
    }

    /// Draws frame `frame` of a [`PiskelSprite`] into the RAM framebuffer.
    pub fn draw_sprite(
        &mut self,
        sprite: &PiskelSprite,
        start_x: i16,
        start_y: i16,
        frame: u16,
    ) {
        if frame >= sprite.frame_count {
            return;
        }

        let w = sprite.width as i16;
        let h = sprite.height as i16;
        let frame_stride = (sprite.width as usize) * (sprite.height as usize);
        let frame_offset = (frame as usize) * frame_stride;

        for py in 0..h {
            let screen_y = start_y + py;
            if screen_y < 0 || screen_y >= SCREEN_H as i16 {
                continue;
            }

            for px in 0..w {
                let screen_x = start_x + px;
                if screen_x < 0 || screen_x >= SCREEN_W as i16 {
                    continue;
                }

                let sprite_pixel_idx =
                    frame_offset + (py as usize) * (sprite.width as usize) + (px as usize);
                let raw_color = sprite.pixels[sprite_pixel_idx];

                if raw_color == TRANSPARENT_KEY {
                    continue;
                }

                let fb_idx = (screen_y as usize) * SCREEN_W + (screen_x as usize);
                self.framebuffer[fb_idx] = raw_color;
            }
        }
    }

    /// Draws a single pixel directly into the RAM framebuffer.
    #[inline]
    pub fn draw_pixel(&mut self, x: i16, y: i16, color: Color) {
        if x >= 0 && y >= 0 && (x as usize) < SCREEN_W && (y as usize) < SCREEN_H {
           self.framebuffer[(y as usize) * SCREEN_W + (x as usize)] = color.0;
        }
    }

    /// Fills a rectangle in the RAM framebuffer.
    pub fn fill_rect(&mut self, x0: i16, y0: i16, x1: i16, y1: i16, color: Color) {
        let raw = color.0;
        let xa = x0.max(0) as usize;
        let ya = y0.max(0) as usize;
        let xb = (x1.min(SCREEN_W as i16 - 1)) as usize;
        let yb = (y1.min(SCREEN_H as i16 - 1)) as usize;

        if xa > xb || ya > yb {
            return;
        }

        for row in ya..=yb {
            let base = row * SCREEN_W;
            self.framebuffer[base + xa..=base + xb].fill(raw);
        }
    }

    // ── Text (synchronous, RAM only — same buffer as other drawing) ──────────

    /// Draws a 5×7 glyph into the RAM framebuffer (no SPI communication).
    /// Returns the x coordinate after the glyph.
    pub fn draw_char(&mut self, x: i16, y: i16, glyph_idx: usize, fg: Color, bg: Color) -> i16 {
        for row in 0..7i16 {
            for col in 0..5i16 {
                let lit = (FONT[glyph_idx][col as usize] >> row) & 1 == 1;
                self.draw_pixel(x + col, y + row, if lit { fg } else { bg });
            }
        }
        x + 6
    }

    /// Draws an ASCII string (+ extended symbols 0x80-0x8C) into the RAM framebuffer.
    /// Returns the x coordinate after the last character.
    pub fn draw_str(&mut self, mut x: i16, y: i16, text: &[u8], fg: Color, bg: Color) -> i16 {
        for &c in text {
            if x + 5 >= SCREEN_W as i16 { break; }
            if let Some(idx) = char_to_glyph(c) {
                x = self.draw_char(x, y, idx, fg, bg);
            } else {
                x = x.saturating_add(6);
            }
        }
        x
    }

    /// Draws a 32-bit unsigned integer into the RAM framebuffer.
    pub fn draw_u32(&mut self, mut x: i16, y: i16, val: u32, fg: Color, bg: Color) -> i16 {
        let mut n = val;
        let mut digits = [0u8; 10];
        let mut count = 0usize;
        loop {
            digits[count] = (n % 10) as u8;
            n /= 10;
            count += 1;
            if n == 0 { break; }
        }
        for i in (0..count).rev() {
            x = self.draw_char(x, y, digits[i] as usize, fg, bg);
        }
        x
    }

    /// Draws an `f32` into the RAM framebuffer with `decimals` digits after the decimal point.
    /// Handles NaN and +/-Inf.
    pub fn draw_f32(&mut self, mut x: i16, y: i16, val: f32, decimals: u8, fg: Color, bg: Color) -> i16 {
        if val.is_nan() { return self.draw_str(x, y, b"NaN", fg, bg); }
        if val.is_infinite() {
            return self.draw_str(x, y, if val > 0.0 { b"+Inf" } else { b"-Inf" }, fg, bg);
        }
        let negative = val < 0.0;
        let mut abs = if negative { -val } else { val };
        if negative { x = self.draw_char(x, y, 10, fg, bg); } // '-'

        let factor = { let mut f = 1u32; for _ in 0..decimals { f *= 10; } f };
        abs += 0.5 / factor as f32;

        let integer = abs as u32;
        x = self.draw_u32(x, y, integer, fg, bg);

        if decimals > 0 {
            x = self.draw_char(x, y, 38, fg, bg); // '.'
            let mut frac = abs - integer as f32;
            let mut digits = [0u8; 8];
            for i in 0..decimals as usize {
                frac *= 10.0;
                let d = frac as u8;
                digits[i] = d;
                frac -= d as f32;
            }
            for i in 0..decimals as usize {
                x = self.draw_char(x, y, digits[i] as usize, fg, bg);
            }
        }
        x
    }

    // ── Sending to the display (asynchronous, SPI) ────────────────────────────

    /// Sends the entire RAM framebuffer to the physical display in a single
    /// continuous SPI stream (opening the window once, not once per pixel).
    pub async fn flush(&mut self) -> Result<(), SPI::Error> {
        self.display
            .blit_u16(0, 0, (SCREEN_W - 1) as u16, (SCREEN_H - 1) as u16, self.framebuffer)
            .await
    }
}