use clap::ValueEnum;

use crate::cli::{Precision, SceneArgs};
use crate::fractal::Fractal;
use crate::params::Params;
use crate::presets;

/// Below this pixel spacing (relative to the view) f64 coordinates run out of precision.
pub const F64_MAX_ZOOM: f64 = 1e13;
/// Perturbation keeps deltas in f64, which underflow beyond this.
pub const DEEP_MAX_ZOOM: f64 = 1e300;
const MAX_ITER_CAP: f64 = 50_000_000.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub enum Scale {
    /// Palette position proportional to the iteration count.
    Linear,
    /// Square root: colour bands widen as iterations grow.
    Sqrt,
    /// Logarithmic: keeps colours calm in very deep zooms.
    Log,
}

/// A fully resolved scene: the preset with command-line overrides applied.
pub struct Scene {
    pub name: String,
    pub fractal: Fractal,
    pub centre_re: String,
    pub centre_im: String,
    pub zoom: f64,
    pub view_height: f64,
    pub power: i32,
    pub param: (f64, f64),
    pub param2: (f64, f64),
    pub flip_y: bool,
    pub iter_base: f64,
    pub iter_per_decade: f64,
    pub bailout: f64,
    pub precision: Precision,
    pub palette: String,
    pub density: f64,
    pub scale: Scale,
    pub offset: f64,
    pub interior: u32,
    pub rotation: f64,
    pub rotation_speed: f64,
    pub palette_cycle: f64,
}

impl Scene {
    pub fn resolve(a: &SceneArgs) -> anyhow::Result<Self> {
        let preset = match (&a.preset, a.fractal) {
            (Some(name), _) => presets::find(name)
                .ok_or_else(|| anyhow::anyhow!("unknown preset {name:?}; see `fractals list`"))?,
            (None, Some(f)) => presets::for_fractal(f),
            (None, None) => presets::for_fractal(Fractal::Mandelbrot),
        };
        let s = Self {
            name: preset.name.to_string(),
            fractal: a.fractal.unwrap_or(preset.fractal),
            centre_re: a.centre_re.clone().unwrap_or_else(|| preset.centre.0.to_string()),
            centre_im: a.centre_im.clone().unwrap_or_else(|| preset.centre.1.to_string()),
            zoom: preset.zoom,
            view_height: a.view_height.unwrap_or(preset.view_height),
            power: a.power.unwrap_or(preset.power),
            param: a.param.unwrap_or(preset.param),
            param2: a.param2.unwrap_or(preset.param2),
            flip_y: a.flip_y.unwrap_or(preset.flip_y),
            iter_base: a.iter_base.unwrap_or(preset.iter_base),
            iter_per_decade: a.iter_per_decade.unwrap_or(preset.iter_per_decade),
            bailout: a.bailout.unwrap_or(preset.bailout),
            precision: a.precision,
            palette: a.palette.clone().unwrap_or_else(|| preset.palette.to_string()),
            density: a.density.unwrap_or(preset.density),
            scale: a.colour_scale.unwrap_or(preset.scale),
            offset: a.colour_offset.unwrap_or(preset.offset),
            interior: a.interior.unwrap_or(preset.interior),
            rotation: preset.rotation,
            rotation_speed: preset.rotation_speed,
            palette_cycle: preset.palette_cycle,
        };
        anyhow::ensure!((2..=16).contains(&s.power), "--power must be between 2 and 16");
        anyhow::ensure!(s.view_height > 0.0 && s.bailout > 0.0 && s.iter_base >= 1.0, "invalid view height, bailout or iterations");
        anyhow::ensure!(
            s.precision != Precision::Deep || s.fractal.perturbable(),
            "--precision deep is only available for mandelbrot, multibrot, tricorn, burning-ship and celtic"
        );
        Ok(s)
    }

    /// Whether frames up to `max_zoom` render in deep (perturbation) mode.
    pub fn deep(&self, max_zoom: f64) -> anyhow::Result<bool> {
        let deep = match self.precision {
            Precision::Deep => true,
            Precision::F64 => false,
            Precision::Auto => self.fractal.perturbable() && max_zoom > 1e11,
        };
        if deep {
            anyhow::ensure!(max_zoom <= DEEP_MAX_ZOOM, "zoom is limited to 1e300");
        } else if max_zoom > F64_MAX_ZOOM {
            eprintln!(
                "warning: {} beyond {F64_MAX_ZOOM:e} exceeds f64 precision; frames will pixelate{}",
                self.fractal.name(),
                if self.fractal.perturbable() { " (use --precision deep)" } else { "" }
            );
        }
        Ok(deep)
    }

    pub fn max_iter(&self, zoom: f64) -> i32 {
        (self.iter_base + self.iter_per_decade * zoom.max(1.0).log10()).min(MAX_ITER_CAP).round() as i32
    }

    /// Kernel parameters for one frame.
    pub fn params(&self, width: i32, height: i32, ssaa: i32, zoom: f64, angle_deg: f64, offset: f64) -> Params {
        let (sin, cos) = angle_deg.to_radians().sin_cos();
        Params {
            centre_re: self.centre_re.trim().parse().unwrap_or(0.0),
            centre_im: self.centre_im.trim().parse().unwrap_or(0.0),
            spacing: self.view_height / (zoom * height as f64),
            cos_rot: cos,
            sin_rot: sin,
            flip_y: if self.flip_y { -1.0 } else { 1.0 },
            a_re: self.param.0,
            a_im: self.param.1,
            b_re: self.param2.0,
            b_im: self.param2.1,
            bailout2: self.bailout * self.bailout,
            density: self.density,
            offset,
            width,
            height,
            y0: 0,
            ssaa,
            fractal: self.fractal as i32,
            power: self.power,
            max_iter: self.max_iter(zoom),
            scale: self.scale as i32,
            palette_len: crate::palette::LUT_SIZE as i32,
            interior: self.interior as i32,
            ref_len: 0,
            fp32: 0,
        }
    }

    /// Bits of precision for the centre at magnifications up to `max_zoom` with `pixels`
    /// samples across the view height.
    pub fn bits(&self, max_zoom: f64, pixels: f64) -> usize {
        ((max_zoom * pixels / self.view_height).max(1.0).log2() + 64.0) as usize
    }
}
