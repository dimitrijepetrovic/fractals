/// Per-frame render parameters, shared by the CPU renderer and the CUDA kernel.
/// Must match `struct Params` in cuda/fractals.cu field for field.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Params {
    /// f64 mode: view centre (deep mode renders offsets from the reference orbit instead).
    pub centre_re: f64,
    pub centre_im: f64,
    /// Complex-plane distance between adjacent output pixels.
    pub spacing: f64,
    pub cos_rot: f64,
    pub sin_rot: f64,
    /// +1, or -1 to mirror the imaginary axis.
    pub flip_y: f64,
    /// Julia/Phoenix constant, Newton/Nova relaxation factor.
    pub a_re: f64,
    pub a_im: f64,
    /// Phoenix second constant.
    pub b_re: f64,
    pub b_im: f64,
    /// Squared escape radius.
    pub bailout2: f64,
    /// Palette position = offset + density * scale(nu).
    pub density: f64,
    pub offset: f64,
    pub width: i32,
    pub height: i32,
    /// First row rendered by this GPU launch.
    pub y0: i32,
    /// Subsamples per axis (antialiasing).
    pub ssaa: i32,
    pub fractal: i32,
    pub power: i32,
    pub max_iter: i32,
    /// 0 linear, 1 sqrt, 2 log.
    pub scale: i32,
    pub palette_len: i32,
    /// 0xRRGGBB
    pub interior: i32,
    /// Reference orbit points; 0 in f64 mode.
    pub ref_len: i32,
    /// Perturbation in float32 (GPU only; see gpu.rs).
    pub fp32: i32,
}

// SAFETY: plain-old-data with the same #[repr(C)] layout as the CUDA struct.
#[cfg(feature = "cuda")]
unsafe impl cudarc::driver::DeviceRepr for Params {}
