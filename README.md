# Fractals

Renders zoom videos into 11 kinds of fractals, on an NVIDIA GPU (CUDA) or the CPU. You control
the target, depth, speed, easing, rotation, colours and resolution. Deep zooms (to 1e300) use
perturbation theory, and a built-in autopilot finds new zoom targets.

```sh
cargo build --release
target/release/fractals list                       # fractals, presets, palettes
target/release/fractals render -p mandelbrot-deep  # -> out/mandelbrot-deep.mp4
```

The Mandelbrot deep-zoom maths comes from `~/git/MandelbrotSet` (perturbation with rebasing,
the float32 GPU kernel, the M(23,2) target). This project is self-contained: nothing here
depends on that repository.

## Requirements

- Rust ≥ 1.85 (edition 2024)
- `ffmpeg` on `PATH` (it encodes the videos and writes the PNG stills)
- For the GPU: CUDA toolkit (`nvcc`) at build time and an NVIDIA driver. Developed with CUDA 12.4
  on an RTX 3080 Ti Laptop GPU. Without CUDA, build with `cargo build --release --no-default-features`.
  The CPU renders the same images, only slower.

`CUDA_ARCH=sm_86 cargo build --release` targets a specific GPU. The default is the local one.

## The fractals

`*` = deep zoom to 1e300 by perturbation; the others iterate in f64, to about 1e13.

| Fractal | Iteration | Notes |
|---|---|---|
| `mandelbrot` * | z ← z² + c, z₀ = 0 | The classic. Its boundary has detail at every scale. Zooms end at spirals (Misiurewicz points), or at *minibrots*, small copies of the whole set. |
| `multibrot` * | z ← zᵈ + c | Generalises the Mandelbrot set to `--power d`: it has d−1-fold symmetry, and its minibrots are copies of the d-version. |
| `tricorn` * | z ← z̄² + c | The "Mandelbar". Anti-holomorphic, with three-fold symmetry. Its mini-tricorns are joined by wavy, non-smooth curves. |
| `burning-ship` * | z ← (\|Re z\| + i\|Im z\|)² + c | Michelin & Kessler (1992). The abs() folds give a ship shape and an "armada" of mini-ships along the needle. It is not conformal: deep views stretch, and a lot of the boundary is chaotic dust. |
| `celtic` * | z ← \|Re z²\| + i Im z² + c | Another abs() variant. Its needle carries snowflake-like crosses. |
| `julia` | z ← zᵈ + k, z₀ = pixel | One Julia set per constant k (`--param`). Self-similar spirals around repelling cycles. Try k = −0.8+0.156i, −0.123+0.745i (rabbit), 0+1i (dendrite), −0.391−0.587i (Siegel disk). |
| `phoenix` | z ← z² + p + q·zₙ₋₁ | Ushiki's Phoenix (1988). Remembering the previous iterate gives flame- and lightning-like filaments. Shown rotated 90°, as is traditional. |
| `newton` | z ← z − a·f(z)/f′(z), f = zᵈ − 1 | Newton's method, coloured by the root each point converges to. The three basins meet only at points where all three meet (Wada-like boundary), so the boundary is an infinitely nested chain of "beads". `--param` sets the relaxation a. |
| `nova` | z ← z − a·f/f′ + c, z₀ = 1 | Newton plus a Mandelbrot-style c (Paul Derbyshire). Shows Newton-like bulbs with small Mandelbrot copies. |
| `magnet` | z ← ((z² + c − 1)/(2z + c − 2))² | From the renormalisation group of the Ising model of magnetism. Points escape or converge to 1, and Mandelbrot-like islands appear. |
| `sierpinski` | fold the triangle onto its corner copies | An IFS fractal rendered as escape-time: each iteration magnifies the corner sub-triangle holding the point 2×. The colour is the depth of the hole a point falls into. |

### Presets

Each preset is a fractal with a zoom target and settings that look good. The targets were
found with `fractals find`, and checked by eye.

| Preset | Zoom | |
|---|---|---|
| `mandelbrot` | 1e50 | spirals into the Misiurewicz point M(23,2) |
| `mandelbrot-seahorse` | 1e10 | Seahorse Valley to a period-897 minibrot |
| `mandelbrot-deep` | 6e37 | Elephant Valley to a period-2443 minibrot |
| `multibrot` | 1.4e6 | cubic Multibrot to a period-92 mini copy |
| `tricorn` | 7e4 | to a period-16 mini-tricorn |
| `burning-ship` | 700 | along the needle to a period-5 ship with masts |
| `celtic`, `celtic-deep` | 700, 2.5e12 | Celtic snowflakes to period-5 and period-12 minis |
| `julia` | 1e10 | into a 21-armed spiral of k = −0.8+0.156i |
| `phoenix` | 1e7 | lightning filaments |
| `newton` | 1e12 | the nested root basins of z³ − 1 |
| `nova` | 1e12 | Nova bulbs and tendrils |
| `magnet` | 1e9 | Mandelbrot-like islands in the magnet set |
| `sierpinski` | 1e12 | along a triangle address spelled by the digits of π |

`--fractal X` alone uses the preset named `X`. Any setting can be overridden. For example,
`-p mandelbrot-deep -f tricorn` iterates the Tricorn at the Mandelbrot target.

## Rendering videos

```sh
fractals render -p newton                                    # 1920x1080, 30 fps, out/newton.mp4
fractals render -p mandelbrot --duration 90 --ease smooth    # slower, eases in and out
fractals render -p julia --speed 2                           # zoom 2x per second (sets the duration)
fractals render -p mandelbrot --zoom 1e20 --duration 20      # stop earlier
fractals render -p magnet --rotation-speed 6 --palette-cycle 0.05 --hold-end 3
fractals render -p tricorn --reverse                         # zoom out
fractals render -p celtic-deep --width 3840 --height 2160 --ssaa 3 --encoder hevc_nvenc
fractals render -f julia --param=-0.123,0.745 --centre-re 0 --centre-im 0 --zoom 5 --palette ice
```

| Option | Meaning |
|---|---|
| `--zoom`, `--start-zoom` | Final and first magnification. Zoom 1 shows `--view-height` units of the complex plane, top to bottom. |
| `--duration`, `--speed` | Zoom length in seconds, or zoom factor per second. Any two of zoom, duration and speed fix the third. The default is 8 s + 1.2 s per 10× of zoom. |
| `--ease` | `linear` (constant zoom speed), `smooth`, `in`, `out`. The speed profile is applied in log scale. |
| `--hold-start`, `--hold-end` | Seconds of still frames before and after the zoom. Rotation and palette cycling keep moving during them. |
| `--rotation`, `--rotation-speed` | Initial angle (degrees) and degrees per second. |
| `--palette`, `--density`, `--colour-scale`, `--colour-offset`, `--palette-cycle`, `--interior` | Colouring: see below. |
| `--iter-base`, `--iter-per-decade` | max_iter = base + per_decade · log10(zoom). |
| `--width`, `--height`, `--fps`, `--ssaa` | Frame size, frame rate, and N×N supersampling (antialiasing; 2 by default, which removes most flicker). |
| `--device` | `auto` (GPU if usable), `gpu`, `cpu`. `--threads` sets the CPU threads. |
| `--precision` | `auto`, `f64` or `deep` (perturbation). |
| `--gpu-fp64` | Keep the GPU in float64 (see [Performance](#performance)). |
| `--encoder`, `--crf` | Any ffmpeg encoder (`libx264` by default, `libx265`, `h264_nvenc`, `ffv1`, …), and the quality for x264/x265. |
| `--no-video` | Render without encoding (benchmark). |

**Colours.** A point that escapes after ν iterations (a smooth, fractional count) gets
palette position `offset + density · scale(ν)`. `--colour-scale` sets `scale`:
- `linear`: ν. Bands stay the same width in iterations.
- `sqrt`: √ν.
- `log`: ln(1+ν). Best for deep zooms, where ν reaches the tens of thousands and linear
  colouring turns into noise.

`--palette-cycle` shifts the offset over time, which animates the colours. Newton colours
each root's basin with its own palette colour, banded by convergence speed. Palettes:
`classic ultra fire ocean rainbow sunset ice neon earth grayscale`.

**Single frames.** `fractals still -p NAME --zoom Z -o frame.png` renders one frame. It takes
the same scene options, and is the quick way to preview a target or tune colours before a long
render.

## Finding your own zoom targets

```sh
fractals find -f mandelbrot --centre-re 0.2925755 --centre-im -0.0149977 --start-zoom 100 --zoom 1e20
```

`find` prints a centre (with enough digits for the depth), a zoom, iteration settings and a
ready-to-run `render` command. It works in two stages:

1. **Boundary autopilot.** It renders a small grid, then moves to the escaping point with the
   most iterations near the middle of the view, and zooms in by `--step`. That point lies on the
   boundary, where the detail is. It prefers "smooth" candidates, whose neighbours have similar
   iteration counts, to stay out of chaotic dust (common in the Burning Ship). `--top N
   --seed S` picks randomly among the N best, for variety. `--window` limits how far it may
   move per step.
2. **Minibrot** (perturbable fractals; skip with `--boundary-only`). It finds the minibrot that
   dominates the final view, and frames it so it fills `--fill` (default 40%) of the frame:
   - **Period:** iterate the centre together with its Jacobian J = ∂zₙ/∂c. The first n at
     which the view's disk, mapped to first order, contains 0 is the period
     ("ball method": |zₙ| < √|det J|·r).
   - **Nucleus:** Newton's method on z_p(c) = 0 in arbitrary precision.
   - **Size:** ≈ 1/(|∂z_p/∂c|·|∏ f′(zᵢ)|^(1/(d−1))).

   The non-analytic Tricorn, Burning Ship and Celtic maps are treated as real 2-D maps with
   2×2 Jacobians, so one implementation covers all five formulas. A minibrot is typically
   about 1/Z² the size of a view at zoom Z, so the final zoom is much deeper than `--zoom`.

Tips:
- Start from a region you like (`--centre-re/--centre-im/--start-zoom`, previewed with
  `still`), with a small `--window`.
- On the real axis the Burning Ship and Celtic sets match the Mandelbrot set. Their needles
  carry mini-ships at the Mandelbrot set's real minibrots (−1.7549 period 3, −1.8608 period 5,
  −1.9408 period 4, …).
- Around a minibrot of period p, about 32·p iterations resolve its shape. `find` suggests
  `--iter-per-decade` from that.

## Performance

Whole videos at the defaults (1920×1080, 30 fps, 2×2 supersampling, libx264 CRF 18) on an
RTX 3080 Ti Laptop GPU:

| Preset | Frames | Time | fps | File |
|---|---:|---:|---:|---:|
| `mandelbrot` (to 1e50) | 2040 | 4 min 40 s | 7.4 | 776 MB |
| `mandelbrot-seahorse` (to 1e10) | 600 | 1 min 54 s | 5.6 | 198 MB |
| `celtic-deep` (to 2.5e12) | 686 | 30 s | 28 | 70 MB |
| `newton` (to 1e12, float64) | 672 | 1 min 36 s | 7.1 | 34 MB |

These runs are limited by rendering, not encoding: with `--no-video`, `mandelbrot` runs at
7.9 fps, so encoding costs about 10%. Ways to go faster or smaller:
- `--ssaa 1` is about 4× faster, at the cost of some shimmer.
- Fractal detail compresses badly: `--crf 23` or `--encoder hevc_nvenc` give much smaller files.

Notes:
- **Float32 on the GPU.** Consumer GeForce GPUs run float64 at 1/64 of the float32 rate, so
  the five perturbable fractals render in float32 by default:
  - Plain float32 perturbation while the pixel spacing is at least 1e-30 (zoom ~1e27). Down
    to that spacing, the offsets fit float's normal range without rescaling.
  - For the Mandelbrot set only, the MandelbrotSet project's two-phase *scaled* kernel takes
    float32 to spacing 2⁻²²⁰ (~1e63).
  - Deeper frames use float64.

  Float32 is 6–18× faster than float64 here (0.35 s → 0.03 s for the seahorse's last frame).
  The frames look the same (PSNR 22–55 dB against float64, from chaotic boundary pixels that
  escape at slightly different iterations). `--gpu-fp64` turns it off.
- **Cost is dominated by interior pixels**, which always run to max_iter. The main
  Mandelbrot cardioid and period-2 bulb are skipped with a closed-form test. I tried
  periodicity checking for the other interiors and removed it: a tolerance loose enough to
  help blacked out exterior points that shadow repelling cycles, which are exactly the
  spiral centres zooms target.
- The f64-only fractals (Julia, Phoenix, Newton, Nova, Magnet, Sierpinski) run in float64
  on the GPU. Their coordinates are absolute, so float32 would pixelate after a few decades
  of zoom.
- Slow presets: `magnet`, `phoenix` and `mandelbrot-deep` need tens of thousands of
  iterations near their targets. Preview them at a lower `--width/--height` first.
- The CPU path (rayon, one row per task) runs about 10–20× slower than the GPU in float64.
  It produces the same images, up to rounding from fused multiply-add on the GPU.

## How it works

- **Escape time + smooth colouring.** ν = n + 1 − log_d(log₂|zₙ|), with escape radius 256
  (larger for Newton, Nova and Magnet). Newton, Nova and Magnet also stop when the orbit
  converges (|Δz|² < 1e-12), with a fractional count for quadratic convergence.
- **Deep zoom (perturbation).** One reference orbit Zₙ is computed at the target in arbitrary
  precision (`astro-float`, log₂(zoom)+64 bits) and rounded to f64. Each pixel iterates only
  its offset δ from it, in f64 or f32:
  - Mandelbrot: δ ← (2Z+δ)δ + δc
  - Multibrot: δ ← δ·Σₖ(Z+δ)ᵏZᵈ⁻¹⁻ᵏ + δc
  - Tricorn: δ ← conj((2Z+δ)δ) + δc
  - Burning Ship and Celtic: the abs() terms use `diffabs(c,d) = |c+d| − |c|`, evaluated
    exactly by cases.

  Glitches are avoided by **rebasing** (Zhuoran): when |Z+δ| < |δ| or the orbit runs out,
  δ ← Z+δ and the reference restarts at Z₀ = 0. This is valid for all five formulas, because
  each iterates from its critical point 0.
- **One kernel for all fractals.** `cuda/fractals.cu` mirrors `src/cpu.rs`. The fractal id is
  uniform across a launch, so the `switch` costs no warp divergence. Frames are launched in
  strips of 128 rows, so deep frames don't trip the display watchdog. The writer thread feeds
  ffmpeg while the next frame renders.

## Layout

```
cuda/fractals.cu   GPU kernel (all fractals, f64 + f32 perturbation)
src/main.rs        commands: list, render, still; the zoom schedule and video pipeline
src/cli.rs         command-line options
src/scene.rs       preset + overrides -> per-frame kernel parameters
src/presets.rs     built-in scenes
src/fractal.rs     the fractal formulas' ids and descriptions
src/params.rs      kernel parameter struct shared with CUDA
src/cpu.rs         CPU renderer (same maths as the kernel), colouring
src/gpu.rs         CUDA renderer (cudarc), float32/float64 selection
src/reference.rs   arbitrary-precision centre and reference orbit
src/find.rs        autopilot
src/newton.rs      minibrot location (period, Newton, size)
src/palette.rs     palettes
src/video.rs       ffmpeg pipe
```

## Ideas for later

- More fractals: Mandelbox/Mandelbulb (3-D ray marching), Lyapunov, Kleinian groups, Collatz.
- Series approximation / bilinear approximation (BLA) to skip iterations in very deep zooms.
- Distance-estimation or slope shading for a 3-D look.
- Keyframed paths (pan between targets, Julia-constant morphing).
