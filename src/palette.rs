//! Colour palettes, sampled into a lookup table that both renderers interpolate.
//! Every palette is cyclic: position t and t + 1 give the same colour.

pub const LUT_SIZE: usize = 1024;

enum Kind {
    /// Inigo Quilez's cosine palette: a + b cos(2 pi (c t + d)) per channel.
    Cosine([f64; 3], [f64; 3], [f64; 3], [f64; 3]),
    /// Evenly spaced colour stops (0xRRGGBB), wrapping from the last back to the first.
    Stops(&'static [u32]),
}

struct Palette {
    name: &'static str,
    about: &'static str,
    kind: Kind,
}

const PALETTES: &[Palette] = &[
    Palette {
        name: "classic",
        about: "smooth blue-orange-purple cosine (the MandelbrotSet reference colours)",
        kind: Kind::Cosine([0.5; 3], [0.5; 3], [1.0; 3], [0.0, 0.15, 0.30]),
    },
    Palette {
        name: "ultra",
        about: "deep blue, white and gold, like Ultra Fractal's default gradient",
        kind: Kind::Stops(&[0x000764, 0x206bcb, 0xedffff, 0xffaa00, 0x000200]),
    },
    Palette {
        name: "fire",
        about: "black, red, orange, yellow, white",
        kind: Kind::Stops(&[0x000000, 0x5c0000, 0xc81e00, 0xff8c00, 0xffe14d, 0xfffbe6, 0xff8c00, 0x7a0a00]),
    },
    Palette {
        name: "ocean",
        about: "navy, teal and foam",
        kind: Kind::Stops(&[0x020b2b, 0x05386b, 0x0f7c8c, 0x5ad1c9, 0xe8fbf5, 0x3aa6b9, 0x0b4f7a]),
    },
    Palette {
        name: "rainbow",
        about: "full saturated hue cycle",
        kind: Kind::Cosine([0.5; 3], [0.5; 3], [1.0; 3], [0.0, 0.33, 0.67]),
    },
    Palette {
        name: "sunset",
        about: "purple, magenta, orange, pale yellow",
        kind: Kind::Stops(&[0x1a0533, 0x5b1170, 0xb5179e, 0xf72585, 0xff8c42, 0xffd97d, 0xff8c42, 0x7209b7]),
    },
    Palette {
        name: "ice",
        about: "cold whites and blues",
        kind: Kind::Stops(&[0x0a1128, 0x1c3f7a, 0x5f9ddb, 0xdff3ff, 0xffffff, 0x9cc7f0, 0x2e5ea8]),
    },
    Palette {
        name: "neon",
        about: "electric cyan, magenta and lime on dark",
        kind: Kind::Cosine([0.5, 0.5, 0.5], [0.5, 0.5, 0.5], [2.0, 1.0, 0.0], [0.5, 0.20, 0.25]),
    },
    Palette {
        name: "earth",
        about: "browns, olive and sand",
        kind: Kind::Cosine([0.5, 0.5, 0.5], [0.5, 0.5, 0.5], [1.0, 1.0, 0.5], [0.8, 0.90, 0.30]),
    },
    Palette {
        name: "grayscale",
        about: "black to white and back",
        kind: Kind::Stops(&[0x000000, 0xffffff]),
    },
];

pub fn names() -> impl Iterator<Item = (&'static str, &'static str)> {
    PALETTES.iter().map(|p| (p.name, p.about))
}

/// The palette as LUT_SIZE RGB triples in [0, 1].
pub fn lut(name: &str) -> anyhow::Result<Vec<f32>> {
    let pal = PALETTES.iter().find(|p| p.name == name).ok_or_else(|| {
        anyhow::anyhow!("unknown palette {name:?} (one of: {})", PALETTES.iter().map(|p| p.name).collect::<Vec<_>>().join(", "))
    })?;
    let mut out = Vec::with_capacity(3 * LUT_SIZE);
    for k in 0..LUT_SIZE {
        let t = k as f64 / LUT_SIZE as f64;
        let rgb = match &pal.kind {
            Kind::Cosine(a, b, c, d) => {
                std::array::from_fn::<f64, 3, _>(|i| a[i] + b[i] * (std::f64::consts::TAU * (c[i] * t + d[i])).cos())
            }
            Kind::Stops(stops) => {
                let x = t * stops.len() as f64;
                let i = x as usize;
                let f = x - i as f64;
                let (c0, c1) = (stops[i], stops[(i + 1) % stops.len()]);
                // smoothstep between stops avoids visible kinks in the gradient
                let f = f * f * (3.0 - 2.0 * f);
                std::array::from_fn(|ch| {
                    let s = 16 - 8 * ch;
                    let (a, b) = (((c0 >> s) & 255) as f64, ((c1 >> s) & 255) as f64);
                    (a + f * (b - a)) / 255.0
                })
            }
        };
        out.extend(rgb.map(|v| v.clamp(0.0, 1.0) as f32));
    }
    Ok(out)
}
