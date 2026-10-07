use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::fractal::Fractal;
use crate::scene::Scale;

/// Renders zoom videos into fractals.
#[derive(Parser)]
#[command(name = "fractals", version)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// List the fractals, presets and palettes.
    List,
    /// Render a zoom video.
    Render(RenderArgs),
    /// Render a single frame to an image (e.g. to preview a scene).
    Still(StillArgs),
    /// Autopilot: search for a deep zoom target near the boundary and print its coordinates.
    Find(FindArgs),
}

/// What to render. Values not given come from the preset.
#[derive(Args, Clone)]
pub struct SceneArgs {
    /// Built-in scene (see `fractals list`); defaults to the preset of --fractal, else "mandelbrot".
    #[arg(long, short)]
    pub preset: Option<String>,
    /// Fractal formula; overrides the preset's.
    #[arg(long, short, value_enum)]
    pub fractal: Option<Fractal>,
    /// Zoom target, real part, as a decimal string.
    #[arg(long, visible_alias = "center-re", allow_hyphen_values = true)]
    pub centre_re: Option<String>,
    /// Zoom target, imaginary part, as a decimal string.
    #[arg(long, visible_alias = "center-im", allow_hyphen_values = true)]
    pub centre_im: Option<String>,
    /// Complex-plane height of the frame at zoom 1.
    #[arg(long)]
    pub view_height: Option<f64>,
    /// Exponent d for multibrot, julia, newton and nova.
    #[arg(long)]
    pub power: Option<i32>,
    /// Formula constant RE,IM: Julia k, Phoenix p, Newton/Nova relaxation a.
    #[arg(long, value_parser = parse_complex, allow_hyphen_values = true)]
    pub param: Option<(f64, f64)>,
    /// Second constant RE,IM: Phoenix q.
    #[arg(long, value_parser = parse_complex, allow_hyphen_values = true)]
    pub param2: Option<(f64, f64)>,
    /// Mirror the imaginary axis.
    #[arg(long)]
    pub flip_y: Option<bool>,
    /// Maximum iterations at zoom 1.
    #[arg(long)]
    pub iter_base: Option<f64>,
    /// Extra iterations per 10x zoom.
    #[arg(long)]
    pub iter_per_decade: Option<f64>,
    /// Escape radius.
    #[arg(long)]
    pub bailout: Option<f64>,
    /// f64 (to ~1e13), deep (perturbation, to 1e300; mandelbrot, multibrot, tricorn,
    /// burning-ship, celtic), or auto (deep when needed).
    #[arg(long, value_enum, default_value = "auto")]
    pub precision: Precision,
    /// Colour palette (see `fractals list`).
    #[arg(long)]
    pub palette: Option<String>,
    /// Palette cycles per iteration (after --colour-scale).
    #[arg(long)]
    pub density: Option<f64>,
    /// How the iteration count maps to the palette.
    #[arg(long, value_enum)]
    pub colour_scale: Option<Scale>,
    /// Palette position at iteration 0.
    #[arg(long, allow_hyphen_values = true)]
    pub colour_offset: Option<f64>,
    /// Colour of points inside the set, as RRGGBB hex.
    #[arg(long, value_parser = parse_hex)]
    pub interior: Option<u32>,
}

#[derive(Args, Clone)]
pub struct FrameArgs {
    #[arg(long, default_value_t = 1920)]
    pub width: i32,
    #[arg(long, default_value_t = 1080)]
    pub height: i32,
    /// Supersampling: N x N samples per pixel (antialiasing; costs N^2).
    #[arg(long, default_value_t = 2)]
    pub ssaa: i32,
    /// Compute device.
    #[arg(long, value_enum, default_value = "auto")]
    pub device: Device,
    /// CPU threads (0 = all cores).
    #[arg(long, default_value_t = 0)]
    pub threads: usize,
    /// Keep the GPU in float64. By default the perturbable fractals render by perturbation
    /// in float32 down to a pixel spacing of 1e-30 (zoom ~1e27; Mandelbrot ~1e63), many
    /// times faster on consumer GPUs and visually the same.
    #[arg(long)]
    pub gpu_fp64: bool,
}

#[derive(Args)]
pub struct RenderArgs {
    #[command(flatten)]
    pub scene: SceneArgs,
    #[command(flatten)]
    pub frame: FrameArgs,
    /// Final magnification (default: the preset's).
    #[arg(long)]
    pub zoom: Option<f64>,
    /// Magnification of the first frame.
    #[arg(long, default_value_t = 1.0)]
    pub start_zoom: f64,
    /// Length of the zoom in seconds (default: 8 s + 1.2 s per 10x zoom, or from --speed).
    #[arg(long)]
    pub duration: Option<f64>,
    /// Zoom speed: magnification factor per second (e.g. 2 doubles every second).
    /// Sets the duration from the zoom range, or the final zoom when --duration is given.
    #[arg(long)]
    pub speed: Option<f64>,
    #[arg(long, default_value_t = 30.0)]
    pub fps: f64,
    /// Speed profile of the zoom (in log scale; `linear` is a constant zoom speed).
    #[arg(long, value_enum, default_value = "linear")]
    pub ease: Ease,
    /// Seconds to hold still before the zoom starts.
    #[arg(long, default_value_t = 0.0)]
    pub hold_start: f64,
    /// Seconds to hold still after the zoom ends.
    #[arg(long, default_value_t = 0.0)]
    pub hold_end: f64,
    /// Initial rotation in degrees (default: the preset's).
    #[arg(long, allow_hyphen_values = true)]
    pub rotation: Option<f64>,
    /// Rotation speed in degrees per second (default: the preset's).
    #[arg(long, allow_hyphen_values = true)]
    pub rotation_speed: Option<f64>,
    /// Palette cycling in cycles per second (default: the preset's).
    #[arg(long, allow_hyphen_values = true)]
    pub palette_cycle: Option<f64>,
    /// Zoom out instead of in.
    #[arg(long)]
    pub reverse: bool,
    /// ffmpeg video encoder, e.g. libx264, libx265, h264_nvenc, hevc_nvenc, ffv1.
    #[arg(long, default_value = "libx264")]
    pub encoder: String,
    /// Quality for libx264/libx265 (lower is better).
    #[arg(long, default_value_t = 18)]
    pub crf: u32,
    /// Output file (default out/<preset>.mp4).
    #[arg(long, short)]
    pub output: Option<String>,
    /// Render only, skip ffmpeg (benchmark).
    #[arg(long)]
    pub no_video: bool,
}

#[derive(Args)]
pub struct StillArgs {
    #[command(flatten)]
    pub scene: SceneArgs,
    #[command(flatten)]
    pub frame: FrameArgs,
    /// Magnification (default: the preset's final zoom).
    #[arg(long)]
    pub zoom: Option<f64>,
    /// Rotation in degrees (default: the preset's).
    #[arg(long, allow_hyphen_values = true)]
    pub rotation: Option<f64>,
    /// Output image (default out/<preset>.png).
    #[arg(long, short)]
    pub output: Option<String>,
}

#[derive(Args)]
pub struct FindArgs {
    #[command(flatten)]
    pub scene: SceneArgs,
    /// Magnification to search down to (default 1e20 for deep-capable fractals, 1e12
    /// otherwise). A minibrot found there is usually much deeper.
    #[arg(long)]
    pub zoom: Option<f64>,
    /// Magnification to start searching from, around the given centre.
    #[arg(long, default_value_t = 1.0)]
    pub start_zoom: f64,
    /// Search grid size (pixels per side).
    #[arg(long, default_value_t = 121)]
    pub grid: i32,
    /// Zoom factor per search step.
    #[arg(long, default_value_t = 4.0)]
    pub step: f64,
    /// Pick randomly among this many best candidates per step (1 = always the best).
    #[arg(long, default_value_t = 1)]
    pub top: usize,
    /// Random seed for --top.
    #[arg(long, default_value_t = 1)]
    pub seed: u64,
    /// Fraction of the view, around its centre, that candidates may come from.
    #[arg(long, default_value_t = 0.5)]
    pub window: f64,
    /// Only follow the boundary; don't end on a minibrot.
    #[arg(long)]
    pub boundary_only: bool,
    /// Fraction of the frame height the minibrot fills at the final zoom.
    #[arg(long, default_value_t = 0.4)]
    pub fill: f64,
    /// CPU threads (0 = all cores).
    #[arg(long, default_value_t = 0)]
    pub threads: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Device {
    /// The GPU if one is usable, else the CPU.
    Auto,
    Cpu,
    Gpu,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Precision {
    Auto,
    F64,
    Deep,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Ease {
    /// Constant zoom speed.
    Linear,
    /// Slow start and end.
    Smooth,
    /// Slow start.
    In,
    /// Slow end.
    Out,
}

impl Ease {
    pub fn apply(self, t: f64) -> f64 {
        match self {
            Ease::Linear => t,
            Ease::Smooth => t * t * (3.0 - 2.0 * t),
            Ease::In => t * t,
            Ease::Out => t * (2.0 - t),
        }
    }
}

fn parse_complex(s: &str) -> Result<(f64, f64), String> {
    let (re, im) = s.split_once(',').ok_or("expected RE,IM")?;
    Ok((re.trim().parse().map_err(|e| format!("{e}"))?, im.trim().parse().map_err(|e| format!("{e}"))?))
}

fn parse_hex(s: &str) -> Result<u32, String> {
    u32::from_str_radix(s.trim_start_matches('#'), 16).map_err(|e| format!("{e}"))
}
