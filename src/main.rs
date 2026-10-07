mod cli;
mod cpu;
mod find;
mod fractal;
#[cfg(feature = "cuda")]
mod gpu;
mod newton;
mod palette;
mod params;
mod presets;
mod reference;
mod scene;
mod video;

use std::io::Write;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use clap::Parser;
use cli::{Cli, Cmd, Device, Ease, FrameArgs, Precision, RenderArgs, StillArgs};
use params::Params;
use reference::BigPoint;
use scene::Scene;

pub trait Renderer {
    /// Renders one frame into rgb (width * height * 3 bytes).
    fn render(&mut self, p: &Params, rgb: &mut [u8]) -> anyhow::Result<()>;
}

fn main() {
    let result = match Cli::parse().cmd {
        Cmd::List => {
            list();
            Ok(())
        }
        Cmd::Render(a) => render(a),
        Cmd::Still(a) => still(a),
        Cmd::Find(a) => find::run(a),
    };
    if let Err(e) = result {
        eprintln!("\nerror: {e:#}");
        std::process::exit(1);
    }
}

fn list() {
    println!("FRACTALS (* = deep zoom to 1e300 by perturbation; others f64, to ~1e13)");
    for f in fractal::ALL {
        println!("  {:<13}{} {}", f.name(), if f.perturbable() { '*' } else { ' ' }, f.formula());
    }
    println!("\nPRESETS (--preset NAME)");
    for p in presets::PRESETS {
        println!("  {:<22} {:<13} zoom {:<7.0e} {}", p.name, p.fractal.name(), p.zoom, p.about);
    }
    println!("\nPALETTES (--palette NAME)");
    for (name, about) in palette::names() {
        println!("  {name:<10} {about}");
    }
}

/// The zoom path: magnification, rotation and palette offset of every frame.
struct Motion {
    fps: f64,
    frames: usize,
    /// Frames before the zoom starts, and frames of the zoom itself.
    hold_frames: usize,
    zoom_frames: usize,
    ln_z0: f64,
    ln_z1: f64,
    ease: Ease,
    rotation: f64,
    rotation_speed: f64,
    offset: f64,
    palette_cycle: f64,
}

impl Motion {
    /// (zoom, rotation in degrees, palette offset) of frame i.
    fn at(&self, i: usize) -> (f64, f64, f64) {
        let sec = i as f64 / self.fps;
        // The zoom's first and last frames sit exactly on the start and final zoom.
        let t = (i as f64 - self.hold_frames as f64) / (self.zoom_frames.max(2) - 1) as f64;
        let t = if self.zoom_frames == 1 { 1.0 } else { t.clamp(0.0, 1.0) };
        let zoom = (self.ln_z0 + self.ease.apply(t) * (self.ln_z1 - self.ln_z0)).exp();
        (zoom, self.rotation + self.rotation_speed * sec, self.offset + self.palette_cycle * sec)
    }
}

/// Reference orbit for frames up to `max_zoom`, or None for plain f64 iteration. The GPU
/// also renders shallow zooms by perturbation, as that works in float32.
fn reference_orbit(scene: &Scene, f: &FrameArgs, max_zoom: f64, deep: bool) -> anyhow::Result<Option<Vec<[f64; 2]>>> {
    let gpu_fp32 = cfg!(feature = "cuda") && f.device != Device::Cpu && !f.gpu_fp64;
    if !(deep || gpu_fp32 && scene.fractal.perturbable() && scene.precision == Precision::Auto) {
        return Ok(None);
    }
    let (height, ssaa) = (f.height, f.ssaa);
    let c = BigPoint::parse(&scene.centre_re, &scene.centre_im, scene.bits(max_zoom, (height * ssaa) as f64))?;
    let orbit = reference::orbit(scene.fractal, scene.power, &c, scene.max_iter(max_zoom), scene.bailout * scene.bailout);
    Ok(Some(orbit))
}

fn renderer(f: &FrameArgs, palette: Vec<f32>, orbit: Option<Vec<[f64; 2]>>) -> anyhow::Result<(Box<dyn Renderer>, &'static str)> {
    #[cfg(feature = "cuda")]
    if f.device != Device::Cpu {
        match gpu::GpuRenderer::new(f.width, f.height, &palette, orbit.as_deref(), !f.gpu_fp64) {
            Ok(g) => return Ok((Box::new(g), "gpu")),
            Err(e) if f.device == Device::Auto => eprintln!("GPU unavailable ({e}), using the CPU"),
            Err(e) => return Err(e.context("could not initialise the GPU")),
        }
    }
    #[cfg(not(feature = "cuda"))]
    anyhow::ensure!(f.device != Device::Gpu, "built without GPU support (the `cuda` feature)");
    Ok((Box::new(cpu::CpuRenderer::new(f.threads, palette, orbit)?), "cpu"))
}

fn check_frame(f: &FrameArgs) -> anyhow::Result<()> {
    anyhow::ensure!(f.width > 0 && f.height > 0, "--width and --height must be positive");
    anyhow::ensure!((1..=8).contains(&f.ssaa), "--ssaa must be between 1 and 8");
    Ok(())
}

fn output_path(given: &Option<String>, scene: &Scene, ext: &str) -> anyhow::Result<String> {
    let path = given.clone().unwrap_or_else(|| format!("out/{}.{ext}", scene.name));
    if let Some(dir) = std::path::Path::new(&path).parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    Ok(path)
}

fn render(a: RenderArgs) -> anyhow::Result<()> {
    check_frame(&a.frame)?;
    let scene = Scene::resolve(&a.scene)?;
    anyhow::ensure!(a.fps > 0.0 && a.start_zoom > 0.0, "--fps and --start-zoom must be positive");
    anyhow::ensure!(a.hold_start >= 0.0 && a.hold_end >= 0.0, "hold times must not be negative");

    // Any two of zoom range, duration and speed determine the third.
    let mut end_zoom = a.zoom.unwrap_or(scene.zoom);
    let seconds = match (a.duration, a.speed) {
        (Some(_), Some(_)) if a.zoom.is_some() => anyhow::bail!("give at most two of --zoom, --duration and --speed"),
        (Some(d), Some(s)) => {
            end_zoom = a.start_zoom * s.powf(d);
            d
        }
        (Some(d), None) => d,
        (None, Some(s)) => {
            anyhow::ensure!(s > 1.0, "--speed must be above 1");
            (end_zoom / a.start_zoom).ln().abs() / s.ln()
        }
        // Default: a steady pace of 1.2 s per 10x zoom, plus 8 s.
        (None, None) => (8.0 + 1.2 * (end_zoom / a.start_zoom).log10().abs()).clamp(10.0, 120.0),
    };
    anyhow::ensure!(seconds > 0.0 && end_zoom > 0.0, "the zoom needs a positive duration and magnification");
    let (z0, z1) = if a.reverse { (end_zoom, a.start_zoom) } else { (a.start_zoom, end_zoom) };
    let hold_frames = (a.hold_start * a.fps).round() as usize;
    let zoom_frames = (seconds * a.fps).round().max(1.0) as usize;
    let motion = Motion {
        fps: a.fps,
        frames: hold_frames + zoom_frames + (a.hold_end * a.fps).round() as usize,
        hold_frames,
        zoom_frames,
        ln_z0: z0.ln(),
        ln_z1: z1.ln(),
        ease: a.ease,
        rotation: a.rotation.unwrap_or(scene.rotation),
        rotation_speed: a.rotation_speed.unwrap_or(scene.rotation_speed),
        offset: scene.offset,
        palette_cycle: a.palette_cycle.unwrap_or(scene.palette_cycle),
    };

    let f = &a.frame;
    let start = Instant::now();
    let max_zoom = z0.max(z1);
    let deep = scene.deep(max_zoom)?;
    let orbit = reference_orbit(&scene, f, max_zoom, deep)?;
    let ref_s = start.elapsed().as_secs_f64();
    let ref_len = orbit.as_ref().map_or(0, |o| o.len() as i32);
    let (mut renderer, device) = renderer(f, palette::lut(&scene.palette)?, orbit)?;

    let output = output_path(&a.output, &scene, "mp4")?;
    let mut video =
        if a.no_video { None } else { Some(video::VideoWriter::video(&output, f.width, f.height, a.fps, &a.encoder, a.crf)?) };
    eprintln!(
        "{}: {} frames, {:.1} s, zoom {z0:.3e} -> {z1:.3e}, {}x{} ssaa {}, {} {}",
        scene.name,
        motion.frames,
        motion.frames as f64 / a.fps,
        f.width,
        f.height,
        f.ssaa,
        device,
        if deep { "deep" } else if ref_len > 0 { "f64 (perturbation)" } else { "f64" }
    );

    // The main thread renders while a writer thread feeds ffmpeg, so encoding overlaps
    // rendering. Buffers circulate between the two through a pair of channels.
    let frame_bytes = f.width as usize * f.height as usize * 3;
    let (full_tx, full_rx) = mpsc::sync_channel::<Vec<u8>>(2);
    let (free_tx, free_rx) = mpsc::channel::<Vec<u8>>();
    for _ in 0..3 {
        free_tx.send(vec![0u8; frame_bytes]).unwrap();
    }
    let render_start = Instant::now();
    let writer = thread::spawn(move || -> anyhow::Result<Option<video::VideoWriter>> {
        for rgb in full_rx {
            if let Some(v) = video.as_mut() {
                v.write(&rgb)?;
            }
            let _ = free_tx.send(rgb);
        }
        Ok(video)
    });
    let mut rendered = Ok(());
    let mut shown = 0.0;
    for i in 0..motion.frames {
        let (zoom, angle, offset) = motion.at(i);
        let mut p = scene.params(f.width, f.height, f.ssaa, zoom, angle, offset);
        p.ref_len = ref_len;
        let Ok(mut rgb) = free_rx.recv() else { break }; // the writer failed
        if let Err(e) = renderer.render(&p, &mut rgb) {
            rendered = Err(e);
            break;
        }
        if full_tx.send(rgb).is_err() {
            break;
        }
        let el = render_start.elapsed().as_secs_f64();
        if el - shown < 0.25 && i + 1 < motion.frames {
            continue;
        }
        shown = el;
        let eta = el / (i + 1) as f64 * (motion.frames - i - 1) as f64;
        eprint!(
            "\rframe {}/{}  zoom {zoom:.2e}  iter {}  {:.1} fps  eta {}   ",
            i + 1,
            motion.frames,
            p.max_iter,
            (i + 1) as f64 / el,
            hms(eta)
        );
        std::io::stderr().flush().ok();
    }
    drop(full_tx);
    eprintln!();
    let video = writer.join().unwrap()?;
    rendered?;
    let render_s = render_start.elapsed().as_secs_f64();
    if let Some(v) = video {
        v.finish()?;
    }
    println!(
        "preset={} fractal={} device={device} precision={} size={}x{} ssaa={} frames={} zoom={max_zoom:.3e} \
         ref_s={ref_s:.3} render_s={render_s:.3} total_s={:.3} render_fps={:.2} output={}",
        scene.name,
        scene.fractal.name(),
        if deep { "deep" } else { "f64" },
        f.width,
        f.height,
        f.ssaa,
        motion.frames,
        start.elapsed().as_secs_f64(),
        motion.frames as f64 / render_s,
        if a.no_video { "-" } else { &output },
    );
    Ok(())
}

fn still(a: StillArgs) -> anyhow::Result<()> {
    check_frame(&a.frame)?;
    let scene = Scene::resolve(&a.scene)?;
    let f = &a.frame;
    let zoom = a.zoom.unwrap_or(scene.zoom);
    anyhow::ensure!(zoom > 0.0, "--zoom must be positive");
    let orbit = reference_orbit(&scene, f, zoom, scene.deep(zoom)?)?;
    let mut p = scene.params(f.width, f.height, f.ssaa, zoom, a.rotation.unwrap_or(scene.rotation), scene.offset);
    p.ref_len = orbit.as_ref().map_or(0, |o| o.len() as i32);
    let (mut renderer, device) = renderer(f, palette::lut(&scene.palette)?, orbit)?;
    let mut rgb = vec![0u8; f.width as usize * f.height as usize * 3];
    let t0 = Instant::now();
    renderer.render(&p, &mut rgb)?;
    let render_s = t0.elapsed().as_secs_f64();
    let output = output_path(&a.output, &scene, "png")?;
    let mut img = video::VideoWriter::image(&output, f.width, f.height)?;
    img.write(&rgb)?;
    img.finish()?;
    println!(
        "{output}: {} zoom {zoom:.3e} max_iter {} on {device} in {render_s:.2} s",
        scene.fractal.name(),
        p.max_iter
    );
    Ok(())
}

fn hms(s: f64) -> String {
    let s = s.round() as u64;
    format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}
