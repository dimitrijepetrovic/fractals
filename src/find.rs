//! Autopilot: finds a zoom target by repeatedly re-centring on the point with the highest
//! iteration count that still escapes (or converges) near the middle of the view. Such
//! points lie on the fractal's boundary, where the detail is. For the perturbable
//! fractals it then locates the minibrot that dominates the final view (see newton.rs),
//! so the zoom ends on a miniature copy of the set.

use rayon::prelude::*;

use crate::cli::{FindArgs, Precision};
use crate::cpu::{self, INTERIOR};
use crate::newton;
use crate::reference::{self, BigPoint};
use crate::scene::{DEEP_MAX_ZOOM, F64_MAX_ZOOM, Scene};

/// Largest relative iteration-count difference to a neighbour for a smooth candidate.
const ROUGHNESS: f64 = 0.2;

pub fn run(a: FindArgs) -> anyhow::Result<()> {
    let scene = Scene::resolve(&a.scene)?;
    let deep_ok = scene.fractal.perturbable() && scene.precision != Precision::F64;
    let minibrot = deep_ok && !a.boundary_only;
    let mut target = a.zoom.unwrap_or(if deep_ok { 1e20 } else { 1e12 });
    if !deep_ok && target > F64_MAX_ZOOM {
        eprintln!("{} is limited to f64 precision: searching to {F64_MAX_ZOOM:e}", scene.fractal.name());
        target = F64_MAX_ZOOM;
    }
    anyhow::ensure!(a.grid >= 9 && a.step > 1.0 && a.top >= 1, "need --grid >= 9, --step > 1 and --top >= 1");
    anyhow::ensure!(a.window > 0.0 && a.window <= 1.0, "--window must be in (0, 1]");
    scene.deep(target)?;

    let g = a.grid;
    // A minibrot near magnification Z is typically about 1/Z^2 in size: double the precision.
    let bits = scene.bits(target, g as f64) * if minibrot { 2 } else { 1 };
    let mut c = BigPoint::parse(&scene.centre_re, &scene.centre_im, bits)?;
    let pool = rayon::ThreadPoolBuilder::new().num_threads(a.threads).build()?;
    let mut rng = a.seed.max(1);
    let mut iter_floor = 0; // from the previous level's iteration counts
    let mut zoom = a.start_zoom;
    let mut needed = 0.0f64; // highest iteration count seen at the deepest level
    while zoom < target {
        let mut p = scene.params(g, g, 1, zoom, 0.0, 0.0);
        p.max_iter = p.max_iter.max(iter_floor);
        let deep = deep_ok && (scene.precision == Precision::Deep || zoom > 1e11);
        let orbit = if deep {
            Some(reference::orbit(scene.fractal, scene.power, &c, p.max_iter, p.bailout2))
        } else {
            (p.centre_re, p.centre_im) = c.to_f64();
            None
        };
        p.ref_len = orbit.as_ref().map_or(0, |o| o.len() as i32);
        let n = (g * g) as usize;
        let samples: Vec<_> = pool.install(|| {
            (0..n).into_par_iter().map(|k| cpu::sample(&p, orbit.as_deref(), k % g as usize, k / g as usize, 0, 0)).collect()
        });

        let half = a.window * g as f64 / 2.0;
        let near = |k: usize| {
            let (x, y) = ((k % g as usize) as f64 + 0.5, (k / g as usize) as f64 + 0.5);
            (x - g as f64 / 2.0).abs() <= half && (y - g as f64 / 2.0).abs() <= half
        };
        // Chaotic "dust" (common in the Burning Ship and Celtic sets) also has high
        // iteration counts, but they jump from pixel to pixel and zooming in shows only
        // noise. Prefer smooth candidates: every outside neighbour within `ROUGHNESS` of nu.
        let smooth = |k: usize| {
            let (x, y, gu) = ((k % g as usize) as isize, (k / g as usize) as isize, g as isize);
            let nu = samples[k].nu;
            (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (x + dx, y + dy))).all(|(nx, ny)| {
                if nx < 0 || ny < 0 || nx >= gu || ny >= gu {
                    return true;
                }
                let s = samples[(ny * gu + nx) as usize];
                s.kind == INTERIOR || (s.nu - nu).abs() <= ROUGHNESS * nu
            })
        };
        let outside: Vec<usize> = (0..n).filter(|&k| samples[k].kind != INTERIOR && near(k)).collect();
        let mut cand: Vec<(f64, usize)> = outside.iter().filter(|&&k| smooth(k)).map(|&k| (samples[k].nu, k)).collect();
        if cand.is_empty() {
            cand = outside.iter().map(|&k| (samples[k].nu, k)).collect();
        }
        let interior = samples.iter().filter(|s| s.kind == INTERIOR).count();
        if cand.is_empty() {
            eprintln!("zoom {zoom:.2e}: no boundary near the centre (inside the set?), stopping here");
            break;
        }
        cand.sort_by(|x, y| y.0.total_cmp(&x.0));
        needed = cand[0].0;
        let pick = if a.top > 1 {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng % a.top.min(cand.len()) as u64) as usize
        } else {
            0
        };
        let (nu, k) = cand[pick];
        eprintln!(
            "zoom {zoom:9.2e}  max_iter {:>8}  best {:>10.1}  interior {:>3.0}%  {}",
            p.max_iter,
            nu,
            100.0 * interior as f64 / n as f64,
            if deep { "deep" } else { "f64" }
        );
        // Keep max_iter well above what the boundary needs, so it resolves.
        // (Bounded, as chaotic regions can need any number.)
        iter_floor = (4.0 * needed).min(20.0 * scene.max_iter(zoom) as f64) as i32;
        let (dr, di) = cpu::point(&p, k % g as usize, k / g as usize, 0, 0);
        c.offset(dr, di);
        zoom = (zoom * a.step).min(target);
    }

    let mut per_decade_need = 1.5 * needed;
    if minibrot {
        let radius_log2 = (0.5 * scene.view_height / zoom).log2();
        let max_period = (50.0 * needed).clamp(10_000.0, 2_000_000.0) as usize;
        eprintln!("looking for a minibrot in the view (period <= {max_period})...");
        match newton::find(scene.fractal, scene.power, &c, radius_log2, max_period) {
            Ok(m) => {
                // Its main body is about 2.2 sizes tall; make it fill `fill` of the frame.
                let z = a.fill * scene.view_height / (2.2 * m.size_log2.exp2());
                eprintln!("minibrot: period {}, size {:.3e}, framed at zoom {z:.3e}", m.period, m.size_log2.exp2());
                if z > DEEP_MAX_ZOOM {
                    eprintln!("it is deeper than the 1e300 limit; keeping the boundary target");
                } else {
                    if scene.bits(z, 2160.0) > bits {
                        eprintln!("warning: centre precision ({bits} bits) is short for zoom {z:.1e}");
                    }
                    (c, zoom) = (m.centre, z);
                    per_decade_need = per_decade_need.max(32.0 * m.period as f64);
                }
            }
            Err(e) => eprintln!("no minibrot found ({e}); keeping the boundary target"),
        }
    }

    let digits = (zoom.log10().max(0.0) + 20.0) as usize;
    let (re, im) = c.format(digits)?;
    let decades = zoom.log10().max(1.0);
    let per_decade = ((per_decade_need - scene.iter_base) / decades).max(scene.iter_per_decade).ceil();
    println!("centre_re = {re}");
    println!("centre_im = {im}");
    println!("zoom      = {zoom:.3e}");
    println!("iter      = {per_decade} per decade");
    let preset = a.scene.preset.as_ref().map_or(String::new(), |p| format!(" --preset {p}"));
    println!(
        "\nfractals render{preset} --fractal {} --centre-re {re} --centre-im {im} --zoom {zoom:.3e} --iter-per-decade {per_decade}",
        scene.fractal.name()
    );
    Ok(())
}
