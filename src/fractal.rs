use clap::ValueEnum;

/// The fractal formulas. The discriminants are the ids used by cuda/fractals.cu.
#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub enum Fractal {
    /// z <- z^2 + c
    Mandelbrot,
    /// z <- z^d + c (--power d)
    Multibrot,
    /// z <- conj(z)^2 + c (the "Mandelbar")
    Tricorn,
    /// z <- (|Re z| + i|Im z|)^2 + c
    BurningShip,
    /// z <- |Re(z^2)| + i Im(z^2) + c
    Celtic,
    /// z <- z^d + k, z0 = pixel (--param k)
    Julia,
    /// z <- z^2 + p + q z_prev, z0 = pixel (--param p --param2 q)
    Phoenix,
    /// Newton's method on z^d - 1, coloured by the root reached (--param = relaxation)
    Newton,
    /// z <- z - a (z^d - 1)/(d z^(d-1)) + c, z0 = 1
    Nova,
    /// Magnet type I: z <- ((z^2 + c - 1)/(2z + c - 2))^2
    Magnet,
    /// Sierpinski triangle as an escape-time fractal
    Sierpinski,
}

pub const ALL: [Fractal; 11] = [
    Fractal::Mandelbrot,
    Fractal::Multibrot,
    Fractal::Tricorn,
    Fractal::BurningShip,
    Fractal::Celtic,
    Fractal::Julia,
    Fractal::Phoenix,
    Fractal::Newton,
    Fractal::Nova,
    Fractal::Magnet,
    Fractal::Sierpinski,
];

pub const MANDELBROT: i32 = Fractal::Mandelbrot as i32;
pub const MULTIBROT: i32 = Fractal::Multibrot as i32;
pub const TRICORN: i32 = Fractal::Tricorn as i32;
pub const BURNING_SHIP: i32 = Fractal::BurningShip as i32;
pub const CELTIC: i32 = Fractal::Celtic as i32;
pub const JULIA: i32 = Fractal::Julia as i32;
pub const PHOENIX: i32 = Fractal::Phoenix as i32;
pub const NEWTON: i32 = Fractal::Newton as i32;
pub const NOVA: i32 = Fractal::Nova as i32;
pub const MAGNET: i32 = Fractal::Magnet as i32;
pub const SIERPINSKI: i32 = Fractal::Sierpinski as i32;

impl Fractal {
    pub fn name(self) -> String {
        self.to_possible_value().unwrap().get_name().to_string()
    }

    /// Whether deep zooms (beyond ~1e13) are supported, by perturbation around a
    /// high-precision reference orbit. These formulas iterate from the critical point 0.
    pub fn perturbable(self) -> bool {
        matches!(self, Fractal::Mandelbrot | Fractal::Multibrot | Fractal::Tricorn | Fractal::BurningShip | Fractal::Celtic)
    }

    pub fn formula(self) -> &'static str {
        match self {
            Fractal::Mandelbrot => "z <- z^2 + c",
            Fractal::Multibrot => "z <- z^d + c",
            Fractal::Tricorn => "z <- conj(z)^2 + c",
            Fractal::BurningShip => "z <- (|Re z| + i|Im z|)^2 + c",
            Fractal::Celtic => "z <- |Re z^2| + i Im z^2 + c",
            Fractal::Julia => "z <- z^d + k, z0 = pixel",
            Fractal::Phoenix => "z <- z^2 + p + q z_prev, z0 = pixel",
            Fractal::Newton => "z <- z - a (z^d - 1) / (d z^(d-1)), z0 = pixel",
            Fractal::Nova => "z <- z - a (z^d - 1) / (d z^(d-1)) + c, z0 = 1",
            Fractal::Magnet => "z <- ((z^2 + c - 1) / (2z + c - 2))^2",
            Fractal::Sierpinski => "fold the triangle onto its corner copies",
        }
    }
}
