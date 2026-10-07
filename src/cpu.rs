//! CPU renderer. The per-pixel maths mirrors cuda/fractals.cu; keep the two in sync.

use rayon::prelude::*;

use crate::Renderer;
use crate::fractal::*;
use crate::params::Params;

pub const INTERIOR: i32 = 0;
pub const ESCAPED: i32 = 1;
pub const CONVERGED: i32 = 2;

const LN2: f64 = std::f64::consts::LN_2;
const TWO_PI: f64 = std::f64::consts::TAU;
const CONVERGE_TOL2: f64 = 1e-12;
const SQRT3: f64 = 1.7320508075688772;
/// The outcome of iterating one point.
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub kind: i32,
    /// Smooth (fractional) iteration count.
    pub nu: f64,
    /// CONVERGED: which attractor, as a palette position in [0, 1); -1 if there is only one.
    pub hue: f64,
}

const INSIDE: Sample = Sample { kind: INTERIOR, nu: 0.0, hue: 0.0 };

#[inline]
fn escaped(n: i32, r2: f64, d: i32) -> Sample {
    let nu = n as f64 + 1.0 - (0.5 * r2.ln() / LN2).ln() / (d as f64).ln();
    Sample { kind: ESCAPED, nu: if nu.is_finite() && nu > 0.0 { nu } else { n as f64 }, hue: 0.0 }
}

/// Fractional iteration count for quadratically converging orbits; `e` is the last
/// squared step, below CONVERGE_TOL2.
#[inline]
fn conv_nu(n: i32, e: f64) -> f64 {
    let k = (e.ln() / CONVERGE_TOL2.ln()).max(1.0);
    n as f64 + 1.0 - k.log2().min(1.0)
}

#[inline]
fn cpow(zr: f64, zi: f64, d: i32) -> (f64, f64) {
    let (mut r, mut i) = (zr, zi);
    for _ in 1..d {
        (r, i) = (r * zr - i * zi, r * zi + i * zr);
    }
    (r, i)
}

/// Index of the d-th root of unity nearest to z, as a fraction of a turn.
fn root_hue(zr: f64, zi: f64, d: i32) -> f64 {
    let k = (zi.atan2(zr) * d as f64 / TWO_PI + 0.5).floor() as i32;
    k.rem_euclid(d) as f64 / d as f64
}

#[inline(always)]
fn escape(mut zr: f64, mut zi: f64, p: &Params, d: i32, step: impl Fn(f64, f64) -> (f64, f64)) -> Sample {
    for n in 1..=p.max_iter {
        (zr, zi) = step(zr, zi);
        let r2 = zr * zr + zi * zi;
        if r2 > p.bailout2 {
            return escaped(n, r2, d);
        }
    }
    INSIDE
}

/// Absolute f64 iteration at c = (cr, ci).
pub fn pixel_f64(p: &Params, cr: f64, ci: f64) -> Sample {
    let d = p.power;
    match p.fractal {
        MANDELBROT => escape(0.0, 0.0, p, 2, |r, i| (r * r - i * i + cr, 2.0 * r * i + ci)),
        MULTIBROT => escape(0.0, 0.0, p, d, |r, i| {
            let (pr, pi) = cpow(r, i, d);
            (pr + cr, pi + ci)
        }),
        TRICORN => escape(0.0, 0.0, p, 2, |r, i| (r * r - i * i + cr, -2.0 * r * i + ci)),
        BURNING_SHIP => escape(0.0, 0.0, p, 2, |r, i| (r * r - i * i + cr, 2.0 * (r * i).abs() + ci)),
        CELTIC => escape(0.0, 0.0, p, 2, |r, i| ((r * r - i * i).abs() + cr, 2.0 * r * i + ci)),
        JULIA => escape(cr, ci, p, d, |r, i| {
            let (pr, pi) = cpow(r, i, d);
            (pr + p.a_re, pi + p.a_im)
        }),
        PHOENIX => {
            let (mut zr, mut zi, mut qr, mut qi) = (cr, ci, 0.0, 0.0); // q: previous z
                    for n in 1..=p.max_iter {
                let nr = zr * zr - zi * zi + p.a_re + p.b_re * qr - p.b_im * qi;
                let ni = 2.0 * zr * zi + p.a_im + p.b_re * qi + p.b_im * qr;
                (qr, qi, zr, zi) = (zr, zi, nr, ni);
                let r2 = zr * zr + zi * zi;
                if r2 > p.bailout2 {
                    return escaped(n, r2, 2);
                }
            }
            INSIDE
        }
        NEWTON | NOVA => {
            // Newton: z <- z - a f/f' from z0 = c.  Nova: z <- z - a f/f' + c from z0 = 1.
            // f(z) = z^d - 1.
            let nova = p.fractal == NOVA;
            let (mut zr, mut zi) = if nova { (1.0, 0.0) } else { (cr, ci) };
            for n in 1..=p.max_iter {
                let (pr, pi) = cpow(zr, zi, d - 1);
                let (fr, fi) = (pr * zr - pi * zi - 1.0, pr * zi + pi * zr);
                let (dr, di) = (d as f64 * pr, d as f64 * pi);
                let den = dr * dr + di * di;
                if den == 0.0 {
                    return INSIDE;
                }
                let (qr, qi) = ((fr * dr + fi * di) / den, (fi * dr - fr * di) / den);
                let (mut sr, mut si) = (p.a_re * qr - p.a_im * qi, p.a_re * qi + p.a_im * qr);
                if nova {
                    (sr, si) = (sr - cr, si - ci);
                }
                (zr, zi) = (zr - sr, zi - si);
                let e = sr * sr + si * si;
                if e < CONVERGE_TOL2 {
                    let hue = if nova { -1.0 } else { root_hue(zr, zi, d) };
                    return Sample { kind: CONVERGED, nu: conv_nu(n, e), hue };
                }
                let r2 = zr * zr + zi * zi;
                if r2 > p.bailout2 {
                    return escaped(n, r2, d);
                }
            }
            INSIDE
        }
        MAGNET => {
            // z <- ((z^2 + c - 1) / (2z + c - 2))^2: escapes, or converges to the fixed point 1.
            let (mut zr, mut zi) = (0.0f64, 0.0f64);
                    for n in 1..=p.max_iter {
                let (ar, ai) = (zr * zr - zi * zi + cr - 1.0, 2.0 * zr * zi + ci);
                let (br, bi) = (2.0 * zr + cr - 2.0, 2.0 * zi + ci);
                let den = br * br + bi * bi;
                let (qr, qi) = ((ar * br + ai * bi) / den, (ai * br - ar * bi) / den);
                (zr, zi) = (qr * qr - qi * qi, 2.0 * qr * qi);
                let r2 = zr * zr + zi * zi;
                if r2 > p.bailout2 {
                    return escaped(n, r2, 2);
                }
                let e = (zr - 1.0) * (zr - 1.0) + zi * zi;
                if e < CONVERGE_TOL2 {
                    return Sample { kind: CONVERGED, nu: conv_nu(n, e), hue: -1.0 };
                }
            }
            INSIDE
        }
        SIERPINSKI => {
            // Barycentric coordinates in the triangle with vertices i, (-sqrt3/2, -1/2),
            // (sqrt3/2, -1/2). Each step scales the corner sub-triangle holding the point up
            // to the whole triangle; a point in the removed middle triangle "escapes".
            let mut u = (1.0 + 2.0 * ci) / 3.0;
            let mut v = (1.0 - ci - SQRT3 * cr) / 3.0;
            let mut w = (1.0 - ci + SQRT3 * cr) / 3.0;
            if u < 0.0 || v < 0.0 || w < 0.0 {
                return Sample { kind: ESCAPED, nu: 0.0, hue: 0.0 };
            }
            for n in 1..=p.max_iter {
                if u >= 0.5 {
                    (u, v, w) = (2.0 * u - 1.0, 2.0 * v, 2.0 * w);
                } else if v >= 0.5 {
                    (u, v, w) = (2.0 * u, 2.0 * v - 1.0, 2.0 * w);
                } else if w >= 0.5 {
                    (u, v, w) = (2.0 * u, 2.0 * v, 2.0 * w - 1.0);
                } else {
                    return Sample { kind: ESCAPED, nu: n as f64 + 1.0 - 2.0 * u.max(v).max(w), hue: 0.0 };
                }
            }
            INSIDE
        }
        _ => INSIDE,
    }
}

/// Whether c is inside the Mandelbrot set's main cardioid or period-2 bulb, which take
/// max_iter iterations otherwise. Only used while pixels are much larger than the test's
/// f64 rounding error.
#[inline]
fn in_cardioid(x: f64, y: f64) -> bool {
    let xq = x - 0.25;
    let q = xq * xq + y * y;
    q * (q + xq) <= 0.25 * y * y || (x + 1.0) * (x + 1.0) + y * y <= 0.0625
}
const CARDIOID_MIN_SPACING: f64 = 1e-12;

/// |c + d| - |c|, exact for tiny d (perturbation of abs()).
#[inline]
fn diffabs(c: f64, d: f64) -> f64 {
    let cd = c + d;
    if c >= 0.0 {
        if cd >= 0.0 { d } else { -(2.0 * c + d) }
    } else if cd > 0.0 {
        2.0 * c + d
    } else {
        -d
    }
}

/// Perturbation: iterate the offset dz from the reference orbit (Z_0 = 0), with rebasing
/// (Zhuoran) to the orbit start when |Z + dz| < |dz| or the orbit runs out.
#[inline(always)]
fn perturb(p: &Params, orbit: &[[f64; 2]], d: i32, step: impl Fn(f64, f64, f64, f64) -> (f64, f64)) -> Sample {
    let last = orbit.len() - 1;
    let (mut dr, mut di, mut m) = (0.0, 0.0, 0usize);
    for n in 1..=p.max_iter {
        // One bounds check for Z_m and Z_m+1.
        let &[[zr, zi], [nr, ni]] = &orbit[m..m + 2] else { unreachable!() };
        (dr, di) = step(zr, zi, dr, di);
        m += 1;
        let (xr, xi) = (nr + dr, ni + di);
        let r2 = xr * xr + xi * xi;
        if r2 > p.bailout2 {
            return escaped(n, r2, d);
        }
        if r2 < dr * dr + di * di || m == last {
            (dr, di, m) = (xr, xi, 0);
        }
    }
    INSIDE
}

/// Perturbed iteration at offset (cr, ci) from the reference orbit's centre.
pub fn pixel_perturb(p: &Params, orbit: &[[f64; 2]], cr: f64, ci: f64) -> Sample {
    let d = p.power;
    match p.fractal {
        MANDELBROT => perturb(p, orbit, 2, |zr, zi, r, i| {
            let (tr, ti) = (2.0 * zr + r, 2.0 * zi + i);
            (tr * r - ti * i + cr, tr * i + ti * r + ci)
        }),
        // (Z+dz)^d - Z^d = dz * sum_{k<d} (Z+dz)^k Z^(d-1-k)
        MULTIBROT => perturb(p, orbit, d, |zr, zi, r, i| {
            let (ar, ai) = (zr + r, zi + i);
            let (mut sr, mut si, mut br, mut bi) = (1.0, 0.0, 1.0, 0.0);
            for _ in 2..=d {
                (br, bi) = (br * zr - bi * zi, br * zi + bi * zr);
                (sr, si) = (sr * ar - si * ai + br, sr * ai + si * ar + bi);
            }
            (r * sr - i * si + cr, r * si + i * sr + ci)
        }),
        TRICORN => perturb(p, orbit, 2, |zr, zi, r, i| {
            let (tr, ti) = (2.0 * zr + r, 2.0 * zi + i);
            (tr * r - ti * i + cr, -(tr * i + ti * r) + ci)
        }),
        BURNING_SHIP => perturb(p, orbit, 2, |zr, zi, r, i| {
            (
                (2.0 * zr + r) * r - (2.0 * zi + i) * i + cr,
                2.0 * diffabs(zr * zi, zr * i + r * zi + r * i) + ci,
            )
        }),
        CELTIC => perturb(p, orbit, 2, |zr, zi, r, i| {
            (
                diffabs(zr * zr - zi * zi, (2.0 * zr + r) * r - (2.0 * zi + i) * i) + cr,
                2.0 * (zr * i + r * zi + r * i) + ci,
            )
        }),
        _ => INSIDE,
    }
}

/// The point (absolute in f64 mode, offset in deep mode) sampled at output pixel (x, y),
/// subsample (sx, sy).
#[inline]
pub fn point(p: &Params, x: usize, y: usize, sx: i32, sy: i32) -> (f64, f64) {
    let inv = 1.0 / p.ssaa as f64;
    let ox = (x as f64 + (sx as f64 + 0.5) * inv - 0.5 * p.width as f64) * p.spacing;
    let oy = (0.5 * p.height as f64 - (y as f64 + (sy as f64 + 0.5) * inv)) * p.spacing * p.flip_y;
    (ox * p.cos_rot - oy * p.sin_rot, ox * p.sin_rot + oy * p.cos_rot)
}

/// Iterates the point at (x, y, sx, sy).
#[inline]
pub fn sample(p: &Params, orbit: Option<&[[f64; 2]]>, x: usize, y: usize, sx: i32, sy: i32) -> Sample {
    let (dr, di) = point(p, x, y, sx, sy);
    if p.fractal == MANDELBROT && p.spacing > CARDIOID_MIN_SPACING && in_cardioid(p.centre_re + dr, p.centre_im + di) {
        return INSIDE;
    }
    match orbit {
        Some(o) => pixel_perturb(p, o, dr, di),
        None => pixel_f64(p, p.centre_re + dr, p.centre_im + di),
    }
}

fn scale_nu(p: &Params, nu: f64) -> f64 {
    match p.scale {
        1 => nu.sqrt(),
        2 => nu.ln_1p(),
        _ => nu,
    }
}

/// Adds k times the colour at palette position t (wrapping) to acc.
fn palette(pal: &[f32], t: f64, k: f32, acc: &mut [f32; 3]) {
    let n = pal.len() / 3;
    let x = (t - t.floor()) * n as f64;
    let i = (x as usize).min(n - 1);
    let j = (i + 1) % n;
    let f = (x - i as f64) as f32;
    for c in 0..3 {
        acc[c] += k * (pal[3 * i + c] + f * (pal[3 * j + c] - pal[3 * i + c]));
    }
}

pub fn colour(p: &Params, pal: &[f32], s: Sample, acc: &mut [f32; 3]) {
    match s.kind {
        ESCAPED => palette(pal, p.offset + p.density * scale_nu(p, s.nu), 1.0, acc),
        // single attractor: colour by convergence speed
        CONVERGED if s.hue < 0.0 => palette(pal, p.offset + 0.5 + p.density * scale_nu(p, s.nu), 1.0, acc),
        CONVERGED => {
            let band = 0.6 + 0.4 * (TWO_PI * p.density * scale_nu(p, s.nu)).cos() as f32;
            palette(pal, p.offset + s.hue, band, acc);
        }
        _ => {
            acc[0] += ((p.interior >> 16) & 255) as f32 / 255.0;
            acc[1] += ((p.interior >> 8) & 255) as f32 / 255.0;
            acc[2] += (p.interior & 255) as f32 / 255.0;
        }
    }
}

pub struct CpuRenderer {
    pool: rayon::ThreadPool,
    palette: Vec<f32>,
    orbit: Option<Vec<[f64; 2]>>,
}

impl CpuRenderer {
    pub fn new(threads: usize, palette: Vec<f32>, orbit: Option<Vec<[f64; 2]>>) -> anyhow::Result<Self> {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build()?;
        Ok(Self { pool, palette, orbit })
    }
}

impl Renderer for CpuRenderer {
    fn render(&mut self, p: &Params, rgb: &mut [u8]) -> anyhow::Result<()> {
        let (pal, orbit) = (&self.palette[..], self.orbit.as_deref());
        let w = p.width as usize;
        let k = 255.0 / (p.ssaa * p.ssaa) as f32;
        // One row per task; rayon's work stealing balances the uneven rows.
        self.pool.install(|| {
            rgb.par_chunks_mut(w * 3).enumerate().for_each(|(y, row)| {
                for (x, px) in row.chunks_exact_mut(3).enumerate() {
                    let mut acc = [0.0f32; 3];
                    for sy in 0..p.ssaa {
                        for sx in 0..p.ssaa {
                            colour(p, pal, sample(p, orbit, x, y, sx, sy), &mut acc);
                        }
                    }
                    for c in 0..3 {
                        px[c] = (acc[c] * k + 0.5).min(255.0) as u8;
                    }
                }
            });
        });
        Ok(())
    }
}
