use std::sync::Arc;

use cudarc::driver::{CudaFunction, CudaSlice, CudaStream, LaunchConfig, PushKernelArg};
use cudarc::nvrtc::Ptx;

use crate::Renderer;
use crate::fractal::MANDELBROT;
use crate::params::Params;

const PTX: &str = include_str!(concat!(env!("OUT_DIR"), "/fractals.ptx"));
const BLOCK: u32 = 16;
/// Smallest pixel spacing for float32 perturbation: the pixel offsets (down to ~1e-35 with
/// supersampling) must stay normal floats (> 1.2e-38). Deeper frames use float64, except
/// the Mandelbrot set, whose scaled float32 kernel works down to SCALED_F32_MIN_SPACING.
/// Both as in cuda/fractals.cu.
const F32_MIN_SPACING: f64 = 1e-30;
/// 2^-220: the scaled deltas must reach 2^-100 without overflowing float.
const SCALED_F32_MIN_SPACING: f64 = f64::from_bits((1023 - 220) << 52);
/// Rows per kernel launch: keeps each launch short, so deep frames with huge iteration
/// counts don't trip the display driver's watchdog.
const ROWS_PER_LAUNCH: u32 = 8 * BLOCK;

pub struct GpuRenderer {
    stream: Arc<CudaStream>,
    kernel: CudaFunction,
    rgb: CudaSlice<u8>,
    palette: CudaSlice<f32>,
    /// Reference orbit, interleaved (re, im); a dummy entry in f64 mode.
    orbit: CudaSlice<f64>,
    /// The orbit rounded to float32.
    orbit32: CudaSlice<f32>,
    fp32: bool,
}

impl GpuRenderer {
    /// `fp32`: render perturbation frames in float32 where precise enough.
    pub fn new(width: i32, height: i32, palette: &[f32], orbit: Option<&[[f64; 2]]>, fp32: bool) -> anyhow::Result<Self> {
        let ctx = cudarc::driver::CudaContext::new(0)?;
        let stream = ctx.default_stream();
        let module = ctx.load_module(Ptx::from_src(PTX))?;
        let flat = orbit.map_or(&[0.0, 0.0][..], |o| o.as_flattened());
        Ok(Self {
            kernel: module.load_function("render")?,
            rgb: stream.alloc_zeros::<u8>(width as usize * height as usize * 3)?,
            palette: stream.clone_htod(palette)?,
            orbit: stream.clone_htod(flat)?,
            orbit32: stream.clone_htod(&flat.iter().map(|&v| v as f32).collect::<Vec<_>>())?,
            fp32,
            stream,
        })
    }
}

impl Renderer for GpuRenderer {
    fn render(&mut self, p: &Params, rgb: &mut [u8]) -> anyhow::Result<()> {
        let (w, h) = (p.width as u32, p.height as u32);
        for y0 in (0..h).step_by(ROWS_PER_LAUNCH as usize) {
            let rows = ROWS_PER_LAUNCH.min(h - y0);
            let cfg = LaunchConfig {
                grid_dim: (w.div_ceil(BLOCK), rows.div_ceil(BLOCK), 1),
                block_dim: (BLOCK, BLOCK, 1),
                shared_mem_bytes: 0,
            };
            let min_spacing = if p.fractal == MANDELBROT { SCALED_F32_MIN_SPACING } else { F32_MIN_SPACING };
            let fp32 = self.fp32 && p.ref_len > 0 && p.spacing >= min_spacing;
            let launch_p = Params { y0: y0 as i32, fp32: fp32 as i32, ..*p };
            let mut launch = self.stream.launch_builder(&self.kernel);
            launch.arg(&mut self.rgb).arg(&self.palette).arg(&self.orbit).arg(&self.orbit32).arg(&launch_p);
            // SAFETY: argument types and order match `render` in cuda/fractals.cu.
            unsafe { launch.launch(cfg) }?;
        }
        self.stream.memcpy_dtoh(&self.rgb, rgb)?;
        Ok(())
    }
}
