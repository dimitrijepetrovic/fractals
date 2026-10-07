//! Built-in scenes: a fractal, a zoom target and the settings that make it look good.
//! Every fractal has a preset with its own name, used when only --fractal is given.

use crate::fractal::Fractal;
use crate::scene::Scale;

pub struct Preset {
    pub name: &'static str,
    pub about: &'static str,
    pub fractal: Fractal,
    /// Zoom target as decimal strings (use many digits for deep zooms).
    pub centre: (&'static str, &'static str),
    /// Default final magnification.
    pub zoom: f64,
    /// Complex-plane height of the frame at zoom 1.
    pub view_height: f64,
    pub power: i32,
    pub param: (f64, f64),
    pub param2: (f64, f64),
    /// Mirror the imaginary axis (shows the Burning Ship upright).
    pub flip_y: bool,
    /// max_iter = iter_base + iter_per_decade * log10(zoom)
    pub iter_base: f64,
    pub iter_per_decade: f64,
    pub bailout: f64,
    pub palette: &'static str,
    pub density: f64,
    pub scale: Scale,
    pub offset: f64,
    pub interior: u32,
    /// Initial rotation in degrees.
    pub rotation: f64,
    /// Degrees per second.
    pub rotation_speed: f64,
    /// Palette cycles per second.
    pub palette_cycle: f64,
}

const BASE: Preset = Preset {
    name: "",
    about: "",
    fractal: Fractal::Mandelbrot,
    centre: ("0", "0"),
    zoom: 1e6,
    view_height: 3.0,
    power: 2,
    param: (0.0, 0.0),
    param2: (0.0, 0.0),
    flip_y: false,
    iter_base: 300.0,
    iter_per_decade: 150.0,
    bailout: 256.0,
    palette: "classic",
    density: 0.02,
    scale: Scale::Linear,
    offset: 0.0,
    interior: 0x000000,
    rotation: 0.0,
    rotation_speed: 0.0,
    palette_cycle: 0.0,
};

/// The Misiurewicz point M(23,2) (preperiod 23, period 2), from the MandelbrotSet project:
/// spirals at every depth, so the zoom never ends in a featureless region.
const M23_2: (&str, &str) = (
    "-0.7766105925997018565640395025529947493281703214398660009692436578520468609045987087869614280320504142913293744385181191871713086676213649222522743166627219099561021923842471419613761736622513496509490647163308646781007447098756656445264983237348114769587849363429595360430698257987637989567458986520611392894629900349384819163368360786901345",
    "0.1346089616750281660567372702330578095411874962204036173399792327554399692618243468637467375372086757134654215617641155866771648880044865998923176610765412471665957254547656437166294650322153138263014932943834549306648786205329836392513593309204091729193444399806460589188944478163945916072585799737463142109726240996892519451085765977598015",
);

pub const PRESETS: &[Preset] = &[
    // ---- Mandelbrot family: deep zooms by perturbation ----
    Preset {
        name: "mandelbrot",
        about: "endless spirals into the Misiurewicz point M(23,2)",
        centre: M23_2,
        zoom: 1e50,
        iter_base: 512.0,
        iter_per_decade: 128.0,
        density: 0.015,
        ..BASE
    },
    Preset {
        name: "mandelbrot-seahorse",
        about: "through Seahorse Valley to a period-897 minibrot",
        centre: ("-0.74364392404504586110885024557", "0.13182591993348176575415944446"),
        zoom: 9.9e9,
        iter_per_decade: 2840.0,
        scale: Scale::Log,
        density: 1.5,
        ..BASE
    },
    Preset {
        name: "mandelbrot-deep",
        about: "from Elephant Valley down to a period-2443 minibrot at 6e37",
        centre: (
            "0.291181378027553995059550899778514088509950435769619239054",
            "-0.016475067436904813805114838673783128082073057178175382104",
        ),
        zoom: 6.4e37,
        iter_per_decade: 2054.0,
        palette: "ultra",
        scale: Scale::Log,
        density: 2.0,
        rotation_speed: 2.0,
        ..BASE
    },
    Preset {
        name: "multibrot",
        about: "the cubic Multibrot z^3 + c, into a period-92 mini copy",
        fractal: Fractal::Multibrot,
        power: 3,
        centre: ("0.46703577608945627133962108", "0.5527883718901412617189813"),
        zoom: 1.4e6,
        iter_per_decade: 430.0,
        palette: "sunset",
        scale: Scale::Log,
        density: 1.5,
        ..BASE
    },
    Preset {
        name: "tricorn",
        about: "the Tricorn (Mandelbar) z <- conj(z)^2 + c, into a period-16 mini tricorn",
        fractal: Fractal::Tricorn,
        centre: ("-1.115811043438464980245475", "0.162890667359523018496643"),
        zoom: 7.1e4,
        palette: "ultra",
        scale: Scale::Log,
        density: 1.5,
        ..BASE
    },
    Preset {
        name: "burning-ship",
        about: "the Burning Ship's needle, to a period-5 ship with masts",
        fractal: Fractal::BurningShip,
        centre: ("-1.8607825222048548712322", "0"),
        zoom: 700.0,
        flip_y: true,
        palette: "fire",
        ..BASE
    },
    Preset {
        name: "celtic",
        about: "the Celtic Mandelbrot's snowflake needle, to a period-5 mini",
        fractal: Fractal::Celtic,
        centre: ("-1.8607825222048548712322", "0"),
        zoom: 699.0,
        scale: Scale::Log,
        density: 1.5,
        ..BASE
    },
    Preset {
        name: "celtic-deep",
        about: "Celtic snowflakes down to a period-12 mini at 3e12",
        fractal: Fractal::Celtic,
        centre: ("-1.99999911758726082503335072858064", "0"),
        zoom: 2.5e12,
        palette: "ice",
        scale: Scale::Log,
        density: 1.5,
        ..BASE
    },
    // ---- f64 fractals (to ~1e13) ----
    Preset {
        name: "julia",
        about: "Julia set of z^2 - 0.8 + 0.156i, into a spiral",
        fractal: Fractal::Julia,
        param: (-0.8, 0.156),
        centre: ("0.1730313692107572470786318596986", "0.23855499475073522533521042577394"),
        zoom: 1e10,
        iter_per_decade: 507.0,
        palette: "ultra",
        scale: Scale::Log,
        density: 1.2,
        ..BASE
    },
    Preset {
        name: "phoenix",
        about: "the Phoenix fractal (p = 0.5667, q = -0.5), along its lightning filaments",
        fractal: Fractal::Phoenix,
        param: (0.5667, 0.0),
        param2: (-0.5, 0.0),
        centre: ("0.13513396598768135408617378457", "-0.65203808744030058035770693474"),
        zoom: 1e7,
        iter_per_decade: 4930.0,
        rotation: 90.0,
        palette: "sunset",
        scale: Scale::Log,
        density: 1.0,
        ..BASE
    },
    Preset {
        name: "newton",
        about: "Newton's method on z^3 - 1: the three root basins, nested forever",
        fractal: Fractal::Newton,
        power: 3,
        param: (1.0, 0.0),
        centre: ("0.11971948118849120654576250514835", "0.06954456945565073449933579623887"),
        zoom: 1e12,
        iter_base: 100.0,
        iter_per_decade: 20.0,
        bailout: 1e12,
        density: 0.08,
        palette: "rainbow",
        ..BASE
    },
    Preset {
        name: "nova",
        about: "the Nova fractal z <- z - (z^3 - 1)/(3z^2) + c",
        fractal: Fractal::Nova,
        power: 3,
        param: (1.0, 0.0),
        centre: ("-0.56414125668416092613222466945368", "0.02313063700440421865953705215158"),
        zoom: 1e12,
        bailout: 1e12,
        density: 0.05,
        palette: "ocean",
        ..BASE
    },
    Preset {
        name: "magnet",
        about: "Magnet type I (from the Ising model), into Mandelbrot-like islands",
        fractal: Fractal::Magnet,
        centre: ("1.16513416196255942093142825379834", "1.30273898553132313594395141935576"),
        zoom: 1e9,
        view_height: 6.0,
        iter_per_decade: 2000.0,
        bailout: 1e6,
        palette: "ice",
        ..BASE
    },
    Preset {
        name: "sierpinski",
        about: "Sierpinski triangle, along an address taken from the digits of pi",
        fractal: Fractal::Sierpinski,
        // sum_k V(d_k mod 3) / 2^k over the digits d_k of pi, V the triangle's corners
        centre: ("-0.3427765074481612018889969", "0.2810201784047777306789484"),
        zoom: 1e12,
        view_height: 2.2,
        iter_base: 20.0,
        iter_per_decade: 4.0,
        density: 0.1,
        palette: "neon",
        ..BASE
    },
];

pub fn find(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.name == name)
}

/// The preset named after a fractal.
pub fn for_fractal(f: Fractal) -> &'static Preset {
    find(&f.name()).expect("every fractal has a preset named after it")
}
