//! Colours the way the Go client keeps them: sRGB bytes, mixed by
//! truncation, so the sky's gradients and the weather's tints come out the
//! same byte for byte.

/// An sRGB colour, 8 bits a channel (raylib's `Color`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// An opaque colour.
pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    Rgba { r, g, b, a: 255 }
}

pub const WHITE: Rgba = rgb(255, 255, 255);

impl Rgba {
    pub const fn with_alpha(self, a: u8) -> Rgba {
        Rgba { a, ..self }
    }

    /// Bevy's colour for this, as the sRGB it is.
    #[cfg(feature = "viewer")]
    pub fn to_bevy(self) -> bevy::color::Color {
        bevy::color::Color::srgba_u8(self.r, self.g, self.b, self.a)
    }

    /// Bevy's linear colour for this, for vertex colours.
    #[cfg(feature = "viewer")]
    pub fn to_linear(self) -> bevy::color::LinearRgba {
        self.to_bevy().to_linear()
    }
}

/// A share `t` of the way from `a` to `b`, opaque.
pub fn mix_colour(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t) as u8;
    rgb(m(a.r, b.r), m(a.g, b.g), m(a.b, b.b))
}

/// `c`'s brightness scaled by `k`.
pub fn brighten(c: Rgba, k: f32) -> Rgba {
    let s = |v: u8| (f32::from(v) * k).min(255.0) as u8;
    Rgba {
        r: s(c.r),
        g: s(c.g),
        b: s(c.b),
        a: c.a,
    }
}

pub fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

/// Go's `float32(math.Pow(float64(a), float64(b)))`.
pub fn powf(a: f32, b: f32) -> f32 {
    f64::from(a).powf(f64::from(b)) as f32
}
