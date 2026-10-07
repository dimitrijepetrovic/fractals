//! Locates "minibrots": the small copies of the set that deep zooms end on. A minibrot of
//! period p has a nucleus c where the critical orbit returns to 0 after p steps, z_p(c) = 0.
//!
//! 1. Period: iterate the view centre c0 with the Jacobian J_n = dz_n/dc. To first order the
//!    view, a disk of radius r around c0, maps to a disk of radius ~sqrt|det J_n| r around
//!    z_n; the first n where that disk contains 0 is the period of the dominant minibrot in
//!    view (the "ball method").
//! 2. Nucleus: Newton's method on z_p(c) = 0 in arbitrary precision, c <- c - J_p^-1 z_p.
//! 3. Size: ~1 / (|dz_p/dc| |dz_p/dz_1|^(1/(d-1))) (the atom-size estimate).
//!
//! The non-analytic formulas (Tricorn, Burning Ship, Celtic) are handled as real 2D maps
//! with 2x2 Jacobians, so the same code covers every perturbable fractal.

use astro_float::{BigFloat, RoundingMode};

use crate::fractal::*;
use crate::reference::{BigPoint, log2_abs};

const RM: RoundingMode = RoundingMode::ToEven;

/// A 2x2 real matrix [[a, b], [c, d]].
#[derive(Clone)]
struct Mat([BigFloat; 4]);

struct Ctx {
    fractal: i32,
    power: i32,
    p: usize,
}

impl Ctx {
    fn mul(&self, a: &BigFloat, b: &BigFloat) -> BigFloat {
        a.mul(b, self.p, RM)
    }
    fn add(&self, a: &BigFloat, b: &BigFloat) -> BigFloat {
        a.add(b, self.p, RM)
    }
    fn sub(&self, a: &BigFloat, b: &BigFloat) -> BigFloat {
        a.sub(b, self.p, RM)
    }
    fn int(&self, v: u64) -> BigFloat {
        BigFloat::from_word(v, self.p)
    }

    /// One step z <- f(z) + c, also returning the Jacobian Df at the old z.
    fn step(&self, x: &BigFloat, y: &BigFloat, c: &BigPoint) -> (BigFloat, BigFloat, Mat) {
        let two = self.int(2);
        let (fx, fy, df) = if self.fractal == MULTIBROT {
            // w = z^(d-1); f = w z; f' = d w
            let (mut wr, mut wi) = (x.clone(), y.clone());
            for _ in 2..self.power {
                (wr, wi) = (self.sub(&self.mul(&wr, x), &self.mul(&wi, y)), self.add(&self.mul(&wr, y), &self.mul(&wi, x)));
            }
            let fx = self.sub(&self.mul(&wr, x), &self.mul(&wi, y));
            let fy = self.add(&self.mul(&wr, y), &self.mul(&wi, x));
            let d = self.int(self.power as u64);
            let (u, v) = (self.mul(&d, &wr), self.mul(&d, &wi));
            (fx, fy, Mat([u.clone(), v.neg(), v, u]))
        } else {
            let re = self.sub(&self.mul(x, x), &self.mul(y, y));
            let xy = self.mul(x, y);
            let im = self.add(&xy, &xy);
            let (x2, y2) = (self.mul(&two, x), self.mul(&two, y));
            match self.fractal {
                TRICORN => (re, im.neg(), Mat([x2.clone(), y2.neg(), y2.neg(), x2.neg()])),
                BURNING_SHIP => {
                    let s = |v: &BigFloat| if xy.is_negative() { v.neg() } else { v.clone() };
                    (re, if im.is_negative() { im.neg() } else { im }, Mat([x2.clone(), y2.neg(), s(&y2), s(&x2)]))
                }
                CELTIC => {
                    let neg = re.is_negative();
                    let s = |v: &BigFloat| if neg { v.neg() } else { v.clone() };
                    (if neg { re.neg() } else { re }, im, Mat([s(&x2), s(&y2.neg()), y2, x2]))
                }
                _ => (re, im, Mat([x2.clone(), y2.neg(), y2, x2])),
            }
        };
        (self.add(&fx, &c.re), self.add(&fy, &c.im), df)
    }

    /// a * b
    fn matmul(&self, a: &Mat, b: &Mat) -> Mat {
        let [a0, a1, a2, a3] = &a.0;
        let [b0, b1, b2, b3] = &b.0;
        Mat([
            self.add(&self.mul(a0, b0), &self.mul(a1, b2)),
            self.add(&self.mul(a0, b1), &self.mul(a1, b3)),
            self.add(&self.mul(a2, b0), &self.mul(a3, b2)),
            self.add(&self.mul(a2, b1), &self.mul(a3, b3)),
        ])
    }

    fn det(&self, m: &Mat) -> BigFloat {
        self.sub(&self.mul(&m.0[0], &m.0[3]), &self.mul(&m.0[1], &m.0[2]))
    }

    fn zero(&self) -> Mat {
        Mat(std::array::from_fn(|_| self.int(0)))
    }

    fn identity(&self) -> Mat {
        Mat([self.int(1), self.int(0), self.int(0), self.int(1)])
    }

    /// J <- Df J + I
    fn advance(&self, df: &Mat, j: &Mat) -> Mat {
        let mut m = self.matmul(df, j);
        m.0[0] = self.add(&m.0[0], &self.int(1));
        m.0[3] = self.add(&m.0[3], &self.int(1));
        m
    }

    /// z_n and J_n at c.
    fn iterate(&self, c: &BigPoint, n: usize) -> (BigFloat, BigFloat, Mat) {
        let (mut x, mut y, mut j) = (self.int(0), self.int(0), self.zero());
        for _ in 0..n {
            let (nx, ny, df) = self.step(&x, &y, c);
            j = self.advance(&df, &j);
            (x, y) = (nx, ny);
        }
        (x, y, j)
    }

    fn log2_norm(&self, x: &BigFloat, y: &BigFloat) -> f64 {
        0.5 * log2_abs(&self.add(&self.mul(x, x), &self.mul(y, y)))
    }
}

/// A minibrot nucleus and its approximate size.
pub struct Minibrot {
    pub centre: BigPoint,
    pub period: usize,
    /// log2 of the size estimate (about the radius of its main body).
    pub size_log2: f64,
}

/// Finds the dominant minibrot within `radius` (as log2) of c0.
pub fn find(fractal: Fractal, power: i32, c0: &BigPoint, radius_log2: f64, max_period: usize) -> Result<Minibrot, String> {
    let ctx = Ctx { fractal: fractal as i32, power, p: c0.bits };

    // 1. Period by the ball method.
    let (mut x, mut y, mut j) = (ctx.int(0), ctx.int(0), ctx.zero());
    let mut period = None;
    for n in 1..=max_period {
        let (nx, ny, df) = ctx.step(&x, &y, c0);
        j = ctx.advance(&df, &j);
        (x, y) = (nx, ny);
        let lz = ctx.log2_norm(&x, &y);
        if lz > 8.0 {
            return Err(format!("the orbit escaped after {n} iterations without finding a period"));
        }
        if lz < radius_log2 + 0.5 * log2_abs(&ctx.det(&j)) {
            period = Some(n);
            break;
        }
    }
    let period = period.ok_or_else(|| format!("no period found within {max_period} iterations"))?;

    // 2. Nucleus by Newton's method.
    let mut c = c0.clone();
    let mut converged = false;
    for _ in 0..80 {
        let (x, y, j) = ctx.iterate(&c, period);
        let det = ctx.det(&j);
        let [j0, j1, j2, j3] = &j.0;
        let dx = ctx.sub(&ctx.mul(j3, &x), &ctx.mul(j1, &y)).div(&det, ctx.p, RM);
        let dy = ctx.sub(&ctx.mul(j0, &y), &ctx.mul(j2, &x)).div(&det, ctx.p, RM);
        c.re = ctx.sub(&c.re, &dx);
        c.im = ctx.sub(&c.im, &dy);
        let step = ctx.log2_norm(&dx, &dy);
        if !step.is_finite() || step < -(ctx.p as f64) + 16.0 {
            converged = true;
            break;
        }
    }
    let moved = ctx.log2_norm(&ctx.sub(&c.re, &c0.re), &ctx.sub(&c.im, &c0.im));
    // (NaN-safe: a NaN distance fails too)
    if !converged || moved.is_nan() || moved > radius_log2 + 2.0 {
        return Err(format!("Newton's method did not converge to a period-{period} nucleus near the view"));
    }

    // 3. Size: 1 / (|dz_p/dc| |L|^(1/(d-1))), L = prod_{i=1}^{p-1} Df(z_i), |M| = sqrt|det M|,
    // d the degree of the critical point (2 except for the Multibrot).
    let (mut x, mut y, mut j, mut l) = (ctx.int(0), ctx.int(0), ctx.zero(), ctx.identity());
    for n in 1..=period {
        let (nx, ny, df) = ctx.step(&x, &y, &c);
        if n > 1 {
            l = ctx.matmul(&df, &l);
        }
        j = ctx.advance(&df, &j);
        (x, y) = (nx, ny);
    }
    let d = if ctx.fractal == MULTIBROT { power as f64 } else { 2.0 };
    let size_log2 = -0.5 * (log2_abs(&ctx.det(&j)) + log2_abs(&ctx.det(&l)) / (d - 1.0));
    if !size_log2.is_finite() {
        return Err(format!("degenerate period-{period} nucleus"));
    }
    Ok(Minibrot { centre: c, period, size_log2 })
}
