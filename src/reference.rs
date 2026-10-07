//! Arbitrary-precision arithmetic for deep zooms: the reference orbit and the zoom centre.

use astro_float::{BigFloat, Consts, Radix, RoundingMode, Sign};

use crate::fractal::*;

const RM: RoundingMode = RoundingMode::ToEven;

/// A point of the complex plane in arbitrary precision.
#[derive(Clone)]
pub struct BigPoint {
    pub re: BigFloat,
    pub im: BigFloat,
    pub bits: usize,
}

impl BigPoint {
    pub fn parse(re: &str, im: &str, bits: usize) -> anyhow::Result<Self> {
        let mut cc = Consts::new()?;
        let p = Self {
            re: BigFloat::parse(re.trim(), Radix::Dec, bits, RM, &mut cc),
            im: BigFloat::parse(im.trim(), Radix::Dec, bits, RM, &mut cc),
            bits,
        };
        anyhow::ensure!(!p.re.is_nan() && !p.im.is_nan(), "invalid centre coordinate {re:?}, {im:?}");
        Ok(p)
    }

    /// Moves the point by an f64 offset.
    pub fn offset(&mut self, dr: f64, di: f64) {
        self.re = self.re.add(&BigFloat::from_f64(dr, self.bits), self.bits, RM);
        self.im = self.im.add(&BigFloat::from_f64(di, self.bits), self.bits, RM);
    }

    pub fn to_f64(&self) -> (f64, f64) {
        (to_f64(&self.re), to_f64(&self.im))
    }

    /// Decimal strings with `digits` significant digits after the point.
    pub fn format(&self, digits: usize) -> anyhow::Result<(String, String)> {
        Ok((format_decimal(&self.re, digits)?, format_decimal(&self.im, digits)?))
    }
}

/// Computes the reference orbit Z_0 = 0 .. Z_max_iter of `fractal` at `c`, rounding each
/// point to f64. Stops early if the orbit escapes.
pub fn orbit(fractal: Fractal, power: i32, c: &BigPoint, max_iter: i32, bailout2: f64) -> Vec<[f64; 2]> {
    let p = c.bits;
    let id = fractal as i32;
    let mul = |a: &BigFloat, b: &BigFloat| a.mul(b, p, RM);
    let abs = |a: BigFloat| if a.is_negative() { a.neg() } else { a };
    let (cr, ci) = (&c.re, &c.im);
    let mut orbit = Vec::with_capacity(max_iter as usize + 1);
    orbit.push([0.0, 0.0]);
    let (mut zr, mut zi) = (BigFloat::from_word(0, p), BigFloat::from_word(0, p));
    for _ in 0..max_iter {
        let (nr, ni) = if id == MULTIBROT {
            let (mut r, mut i) = (zr.clone(), zi.clone());
            for _ in 1..power {
                (r, i) = (mul(&r, &zr).sub(&mul(&i, &zi), p, RM), mul(&r, &zi).add(&mul(&i, &zr), p, RM));
            }
            (r, i)
        } else {
            let re = mul(&zr, &zr).sub(&mul(&zi, &zi), p, RM);
            let zrzi = mul(&zr, &zi);
            let im = zrzi.add(&zrzi, p, RM);
            match id {
                TRICORN => (re, im.neg()),
                BURNING_SHIP => (re, abs(im)),
                CELTIC => (abs(re), im),
                _ => (re, im),
            }
        };
        zr = nr.add(cr, p, RM);
        zi = ni.add(ci, p, RM);
        let (dr, di) = (to_f64(&zr), to_f64(&zi));
        orbit.push([dr, di]);
        if dr * dr + di * di > bailout2 {
            break; // the reference escaped: shorter orbit
        }
    }
    orbit
}

/// astro-float has no f64 conversion: take the top mantissa word and scale it.
pub fn to_f64(x: &BigFloat) -> f64 {
    match x.as_raw_parts() {
        Some((words, _, sign, exp, _)) if !x.is_zero() => {
            let top = *words.last().unwrap() as f64; // value = 0.mantissa * 2^exp
            let v = top * 2f64.powi(exp - 64);
            if sign == Sign::Neg { -v } else { v }
        }
        _ => 0.0,
    }
}

/// Plain decimal notation (no exponent), `digits` digits after the point.
fn format_decimal(x: &BigFloat, digits: usize) -> anyhow::Result<String> {
    let mut cc = Consts::new()?;
    let s = x.format(Radix::Dec, RM, &mut cc)?;
    // astro-float gives [-]d.ddd[e[+-]N]
    let (neg, s) = s.strip_prefix('-').map_or((false, s.as_str()), |r| (true, r));
    let (mant, exp) = s.split_once(['e', 'E']).map_or((s, 0), |(m, e)| (m, e.parse::<i64>().unwrap_or(0)));
    let (int, frac) = mant.split_once('.').unwrap_or((mant, ""));
    let all: String = format!("{int}{frac}");
    let point = int.len() as i64 + exp; // digits before the decimal point
    let (int_part, frac_part) = if point <= 0 {
        ("0".to_string(), format!("{}{}", "0".repeat((-point) as usize), all))
    } else if point as usize >= all.len() {
        (format!("{all}{}", "0".repeat(point as usize - all.len())), String::new())
    } else {
        (all[..point as usize].to_string(), all[point as usize..].to_string())
    };
    let int_part = int_part.trim_start_matches('0');
    let mut frac_part: String = frac_part.chars().take(digits).collect();
    while frac_part.ends_with('0') {
        frac_part.pop();
    }
    let int_part = if int_part.is_empty() { "0" } else { int_part };
    let sign = if neg && (int_part != "0" || !frac_part.is_empty()) { "-" } else { "" };
    Ok(if frac_part.is_empty() { format!("{sign}{int_part}") } else { format!("{sign}{int_part}.{frac_part}") })
}

/// log2 |x|, valid far outside the f64 range; -inf for 0.
pub fn log2_abs(x: &BigFloat) -> f64 {
    match x.as_raw_parts() {
        Some((words, _, _, exp, _)) if !x.is_zero() => {
            let top = *words.last().unwrap() as f64 / 2f64.powi(64); // in [0.5, 1)
            exp as f64 + top.log2()
        }
        _ => f64::NEG_INFINITY,
    }
}
