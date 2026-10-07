// GPU version of src/cpu.rs: the same per-pixel maths, keep the two in sync.
// One kernel renders every fractal; the fractal id is uniform across the launch, so the
// switch costs no divergence.

// Must match the ids in src/fractal.rs.
enum {
    MANDELBROT, MULTIBROT, TRICORN, BURNING_SHIP, CELTIC,
    JULIA, PHOENIX, NEWTON, NOVA, MAGNET, SIERPINSKI
};
// Sample kinds, as in src/cpu.rs.
enum { INTERIOR, ESCAPED, CONVERGED };

#define LN2 0.6931471805599453
#define TWO_PI 6.283185307179586
#define CONVERGE_TOL2 1e-12
#define SQRT3 1.7320508075688772

// Must match `Params` in src/params.rs field for field.
struct Params {
    double centre_re, centre_im;  // f64 mode: view centre (deep mode: offsets only)
    double spacing;               // complex-plane distance between output pixels
    double cos_rot, sin_rot;      // view rotation
    double flip_y;                // +1, or -1 to mirror the imaginary axis
    double a_re, a_im;            // Julia/Phoenix constant, Newton/Nova relaxation
    double b_re, b_im;            // Phoenix second constant
    double bailout2;              // squared escape radius
    double density, offset;       // palette position = offset + density * scale(nu)
    int width, height, y0;        // frame size; first row of this launch
    int ssaa;                     // subsamples per axis
    int fractal, power, max_iter;
    int scale;                    // 0 linear, 1 sqrt, 2 log
    int palette_len;              // palette entries (RGB float triples)
    int interior;                 // 0xRRGGBB
    int ref_len;                  // reference orbit points (0: f64 mode)
    int fp32;                     // perturbation in float (GPU only)
};

// hue: the attractor reached (CONVERGED), as a palette position; -1 if there is only one.
struct Sample { int kind; double nu, hue; };

__device__ static Sample interior() { return Sample{INTERIOR, 0.0, 0.0}; }

__device__ static Sample escaped(int n, double r2, int d) {
    double nu = n + 1.0 - log(0.5 * log(r2) / LN2) / log((double)d);
    return Sample{ESCAPED, isfinite(nu) && nu > 0.0 ? nu : (double)n, 0.0};
}

// Fractional iteration count for quadratically converging orbits; `e` is the last
// squared step, below CONVERGE_TOL2.
__device__ static double conv_nu(int n, double e) {
    double k = fmax(log(e) / log(CONVERGE_TOL2), 1.0);
    return n + 1.0 - fmin(log2(k), 1.0);
}

__device__ static void cpow(double zr, double zi, int d, double* pr, double* pi) {
    double r = zr, i = zi;
    for (int k = 1; k < d; ++k) {
        double t = r * zr - i * zi;
        i = r * zi + i * zr;
        r = t;
    }
    *pr = r; *pi = i;
}

// Index of the d-th root of unity nearest to z, as a fraction of a turn.
__device__ static double root_hue(double zr, double zi, int d) {
    int k = (int)floor(atan2(zi, zr) * d / TWO_PI + 0.5);
    return (double)(((k % d) + d) % d) / d;
}

// Escape-time loop for z <- step(z) from z0; STEP is a functor (re, im) -> (re, im).
template <typename Step>
__device__ static Sample escape(double zr, double zi, const Params& p, int d, Step step) {
    for (int n = 1; n <= p.max_iter; ++n) {
        step(zr, zi);
        double r2 = zr * zr + zi * zi;
        if (r2 > p.bailout2) return escaped(n, r2, d);
    }
    return interior();
}

// Absolute f64 iteration at c = (cr, ci).
__device__ static Sample pixel_f64(const Params& p, double cr, double ci) {
    const int d = p.power;
    switch (p.fractal) {
    case MANDELBROT:
        return escape(0.0, 0.0, p, 2, [&](double& r, double& i) {
            double t = r * r - i * i + cr; i = 2.0 * r * i + ci; r = t; });
    case MULTIBROT:
        return escape(0.0, 0.0, p, d, [&](double& r, double& i) {
            double pr, pi; cpow(r, i, d, &pr, &pi); r = pr + cr; i = pi + ci; });
    case TRICORN:
        return escape(0.0, 0.0, p, 2, [&](double& r, double& i) {
            double t = r * r - i * i + cr; i = -2.0 * r * i + ci; r = t; });
    case BURNING_SHIP:
        return escape(0.0, 0.0, p, 2, [&](double& r, double& i) {
            double t = r * r - i * i + cr; i = 2.0 * fabs(r * i) + ci; r = t; });
    case CELTIC:
        return escape(0.0, 0.0, p, 2, [&](double& r, double& i) {
            double t = fabs(r * r - i * i) + cr; i = 2.0 * r * i + ci; r = t; });
    case JULIA:
        return escape(cr, ci, p, d, [&](double& r, double& i) {
            double pr, pi; cpow(r, i, d, &pr, &pi); r = pr + p.a_re; i = pi + p.a_im; });
    case PHOENIX: {
        double zr = cr, zi = ci, qr = 0.0, qi = 0.0;  // q: previous z
        for (int n = 1; n <= p.max_iter; ++n) {
            double nr = zr * zr - zi * zi + p.a_re + p.b_re * qr - p.b_im * qi;
            double ni = 2.0 * zr * zi + p.a_im + p.b_re * qi + p.b_im * qr;
            qr = zr; qi = zi; zr = nr; zi = ni;
            double r2 = zr * zr + zi * zi;
            if (r2 > p.bailout2) return escaped(n, r2, 2);
        }
        return interior();
    }
    case NEWTON:
    case NOVA: {
        // Newton: z <- z - a f/f' from z0 = c.  Nova: z <- z - a f/f' + c from z0 = 1.
        // f(z) = z^d - 1.
        const bool nova = p.fractal == NOVA;
        double zr = nova ? 1.0 : cr, zi = nova ? 0.0 : ci;
        for (int n = 1; n <= p.max_iter; ++n) {
            double pr, pi; cpow(zr, zi, d - 1, &pr, &pi);  // z^(d-1)
            double fr = pr * zr - pi * zi - 1.0, fi = pr * zi + pi * zr;
            double dr = d * pr, di = d * pi;
            double den = dr * dr + di * di;
            if (den == 0.0) return interior();
            double qr = (fr * dr + fi * di) / den, qi = (fi * dr - fr * di) / den;
            double sr = p.a_re * qr - p.a_im * qi, si = p.a_re * qi + p.a_im * qr;
            if (nova) { sr -= cr; si -= ci; }
            zr -= sr; zi -= si;
            double e = sr * sr + si * si;
            if (e < CONVERGE_TOL2)
                return Sample{CONVERGED, conv_nu(n, e), nova ? -1.0 : root_hue(zr, zi, d)};
            double r2 = zr * zr + zi * zi;
            if (r2 > p.bailout2) return escaped(n, r2, d);
        }
        return interior();
    }
    case MAGNET: {
        // z <- ((z^2 + c - 1) / (2z + c - 2))^2: escapes, or converges to the fixed point 1.
        double zr = 0.0, zi = 0.0;
        for (int n = 1; n <= p.max_iter; ++n) {
            double ar = zr * zr - zi * zi + cr - 1.0, ai = 2.0 * zr * zi + ci;
            double br = 2.0 * zr + cr - 2.0, bi = 2.0 * zi + ci;
            double den = br * br + bi * bi;
            double qr = (ar * br + ai * bi) / den, qi = (ai * br - ar * bi) / den;
            zr = qr * qr - qi * qi; zi = 2.0 * qr * qi;
            double r2 = zr * zr + zi * zi;
            if (r2 > p.bailout2) return escaped(n, r2, 2);
            double e = (zr - 1.0) * (zr - 1.0) + zi * zi;
            if (e < CONVERGE_TOL2) return Sample{CONVERGED, conv_nu(n, e), -1.0};
        }
        return interior();
    }
    case SIERPINSKI: {
        // Barycentric coordinates in the triangle with vertices i, (-sqrt3/2, -1/2),
        // (sqrt3/2, -1/2). Each step scales the corner sub-triangle holding the point up to
        // the whole triangle; a point in the removed middle triangle "escapes".
        double u = (1.0 + 2.0 * ci) / 3.0;
        double v = (1.0 - ci - SQRT3 * cr) / 3.0;
        double w = (1.0 - ci + SQRT3 * cr) / 3.0;
        if (u < 0.0 || v < 0.0 || w < 0.0) return Sample{ESCAPED, 0.0, 0.0};
        for (int n = 1; n <= p.max_iter; ++n) {
            if (u >= 0.5) { u = 2.0 * u - 1.0; v *= 2.0; w *= 2.0; }
            else if (v >= 0.5) { v = 2.0 * v - 1.0; u *= 2.0; w *= 2.0; }
            else if (w >= 0.5) { w = 2.0 * w - 1.0; u *= 2.0; v *= 2.0; }
            else return Sample{ESCAPED, n + 1.0 - 2.0 * fmax(u, fmax(v, w)), 0.0};
        }
        return interior();
    }
    }
    return interior();
}

// Whether c is inside the Mandelbrot set's main cardioid or period-2 bulb, which take
// max_iter iterations otherwise. Only used while pixels are much larger than the test's
// f64 rounding error.
__device__ static bool in_cardioid(double x, double y) {
    double xq = x - 0.25, q = xq * xq + y * y;
    return q * (q + xq) <= 0.25 * y * y || (x + 1.0) * (x + 1.0) + y * y <= 0.0625;
}
#define CARDIOID_MIN_SPACING 1e-12

// |c + d| - |c|, exact for tiny d (perturbation of abs()).
template <typename T>
__device__ static T diffabs(T c, T d) {
    T cd = c + d;
    if (c >= T(0)) return cd >= T(0) ? d : -(T(2) * c + d);
    return cd > T(0) ? T(2) * c + d : -d;
}

// Perturbation: iterate the offset dz from the reference orbit ref (Z_0 = 0), with
// rebasing (Zhuoran) to the orbit start when |Z + dz| < |dz| or the orbit runs out.
// T is double, or float (with the orbit rounded to float) while the pixel offsets stay
// in float's normal range; consumer GPUs run float up to 64x faster than double.
template <typename T, typename Step>
__device__ static Sample perturb(const Params& p, const T* ref, int d, Step step) {
    T dr = T(0), di = T(0);
    int m = 0;
    for (int n = 1; n <= p.max_iter; ++n) {
        step(ref[2 * m], ref[2 * m + 1], dr, di);
        ++m;
        T zr = ref[2 * m] + dr, zi = ref[2 * m + 1] + di;
        T r2 = zr * zr + zi * zi;
        if (r2 > T(p.bailout2)) return escaped(n, (double)r2, d);
        if (r2 < dr * dr + di * di || m == p.ref_len - 1) { dr = zr; di = zi; m = 0; }
    }
    return interior();
}

template <typename T>
__device__ static Sample pixel_perturb(const Params& p, const T* ref, T cr, T ci) {
    const int d = p.power;
    switch (p.fractal) {
    case MANDELBROT:
        return perturb(p, ref, 2, [&](T Zr, T Zi, T& r, T& i) {
            T tr = T(2) * Zr + r, ti = T(2) * Zi + i;
            T t = tr * r - ti * i + cr; i = tr * i + ti * r + ci; r = t; });
    case MULTIBROT:
        // (Z+dz)^d - Z^d = dz * sum_{k<d} (Z+dz)^k Z^(d-1-k)
        return perturb(p, ref, d, [&](T Zr, T Zi, T& r, T& i) {
            T ar = Zr + r, ai = Zi + i, sr = T(1), si = T(0), br = T(1), bi = T(0);
            for (int j = 2; j <= d; ++j) {
                T t = br * Zr - bi * Zi; bi = br * Zi + bi * Zr; br = t;
                t = sr * ar - si * ai + br; si = sr * ai + si * ar + bi; sr = t;
            }
            T t = r * sr - i * si + cr; i = r * si + i * sr + ci; r = t; });
    case TRICORN:
        return perturb(p, ref, 2, [&](T Zr, T Zi, T& r, T& i) {
            T tr = T(2) * Zr + r, ti = T(2) * Zi + i;
            T t = tr * r - ti * i + cr; i = -(tr * i + ti * r) + ci; r = t; });
    case BURNING_SHIP:
        return perturb(p, ref, 2, [&](T Zr, T Zi, T& r, T& i) {
            T t = (T(2) * Zr + r) * r - (T(2) * Zi + i) * i + cr;
            i = T(2) * diffabs(Zr * Zi, Zr * i + r * Zi + r * i) + ci; r = t; });
    case CELTIC:
        return perturb(p, ref, 2, [&](T Zr, T Zi, T& r, T& i) {
            T t = diffabs(Zr * Zr - Zi * Zi, (T(2) * Zr + r) * r - (T(2) * Zi + i) * i) + cr;
            i = T(2) * (Zr * i + r * Zi + r * i) + ci; r = t; });
    }
    return interior();
}

// Mandelbrot perturbation in float32 for frames too deep for plain float deltas
// (spacing < F32_MIN_SPACING, down to 2^-220), from the MandelbrotSet project. Phase 1
// keeps the deltas divided by 2^k (spacing = f 2^k): the recurrence is linear in that
// scale, w <- (2Z + dz) w + dc/2^k, with dz = w 2^k applied as two float multiplies (dz
// may round to 0, but is then negligible next to 2Z). Phase 2 continues with plain float
// deltas once |dz| >= 2^-100 (dc may now round to 0, but is then negligible next to dz).
#define F32_MIN_SPACING 1e-30
__device__ static Sample mandel_scaled_f32(const Params& p, const float* ref, double dcr, double dci) {
    int k;
    frexp(p.spacing, &k);
    const float s1 = ldexpf(1.0f, k / 2), s2 = ldexpf(1.0f, k - k / 2);
    const float ur = (float)ldexp(dcr, -k), ui = (float)ldexp(dci, -k);
    const float wmax = ldexpf(1.0f, -100 - k);
    const float bailout2 = (float)p.bailout2;
    float wr = 0.0f, wi = 0.0f, zr = 0.0f, zi = 0.0f, r2 = 0.0f;
    int m = 0, n = 1;
    for (; n <= p.max_iter; ++n) {
        float tr = 2.0f * ref[2 * m] + wr * s1 * s2, ti = 2.0f * ref[2 * m + 1] + wi * s1 * s2;
        float nr = tr * wr - ti * wi + ur;
        wi = tr * wi + ti * wr + ui;
        wr = nr;
        ++m;
        zr = ref[2 * m] + wr * s1 * s2;
        zi = ref[2 * m + 1] + wi * s1 * s2;
        r2 = zr * zr + zi * zi;
        if (r2 > bailout2) return escaped(n, r2, 2);
        if (fabsf(wr) >= wmax || fabsf(wi) >= wmax || m == p.ref_len - 1) break;
    }
    if (n > p.max_iter) return interior();

    float dzr = wr * s1 * s2, dzi = wi * s1 * s2;
    const float fdcr = (float)dcr, fdci = (float)dci;
    if (r2 < dzr * dzr + dzi * dzi || m == p.ref_len - 1) {  // finish phase 1's last iteration
        dzr = zr; dzi = zi; m = 0;
    }
    for (++n; n <= p.max_iter; ++n) {
        float tr = 2.0f * ref[2 * m] + dzr, ti = 2.0f * ref[2 * m + 1] + dzi;
        float nr = tr * dzr - ti * dzi + fdcr;
        dzi = tr * dzi + ti * dzr + fdci;
        dzr = nr;
        ++m;
        zr = ref[2 * m] + dzr;
        zi = ref[2 * m + 1] + dzi;
        r2 = zr * zr + zi * zi;
        if (r2 > bailout2) return escaped(n, r2, 2);
        if (r2 < dzr * dzr + dzi * dzi || m == p.ref_len - 1) { dzr = zr; dzi = zi; m = 0; }
    }
    return interior();
}

__device__ static double scale_nu(const Params& p, double nu) {
    return p.scale == 1 ? sqrt(nu) : p.scale == 2 ? log1p(nu) : nu;
}

// Adds the colour at palette position t (wrapping) to acc, times k.
__device__ static void palette(const Params& p, const float* pal, double t, float k, float* acc) {
    double x = (t - floor(t)) * p.palette_len;
    int i = min((int)x, p.palette_len - 1), j = (i + 1) % p.palette_len;
    float f = (float)(x - i);
    for (int c = 0; c < 3; ++c) acc[c] += k * (pal[3 * i + c] + f * (pal[3 * j + c] - pal[3 * i + c]));
}

__device__ static void colour(const Params& p, const float* pal, Sample s, float* acc) {
    if (s.kind == ESCAPED) {
        palette(p, pal, p.offset + p.density * scale_nu(p, s.nu), 1.0f, acc);
    } else if (s.kind == CONVERGED && s.hue < 0.0) {  // single attractor: colour by speed
        palette(p, pal, p.offset + 0.5 + p.density * scale_nu(p, s.nu), 1.0f, acc);
    } else if (s.kind == CONVERGED) {
        float band = 0.6f + 0.4f * (float)cos(TWO_PI * p.density * scale_nu(p, s.nu));
        palette(p, pal, p.offset + s.hue, band, acc);
    } else {
        acc[0] += ((p.interior >> 16) & 255) / 255.0f;
        acc[1] += ((p.interior >> 8) & 255) / 255.0f;
        acc[2] += (p.interior & 255) / 255.0f;
    }
}

// Renders rows y0 .. y0 + gridDim.y*blockDim.y of the frame into rgb (whole-frame buffer).
// `ref` is the reference orbit as interleaved (re, im) pairs, used when p.ref_len > 0,
// and `ref32` the same rounded to float, used when p.fp32 is set.
extern "C" __global__ void render(unsigned char* rgb, const float* pal, const double* ref,
                                  const float* ref32, Params p) {
    int x = blockIdx.x * blockDim.x + threadIdx.x;
    int y = p.y0 + blockIdx.y * blockDim.y + threadIdx.y;
    if (x >= p.width || y >= p.height) return;

    float acc[3] = {0.0f, 0.0f, 0.0f};
    const double inv = 1.0 / p.ssaa;
    for (int sy = 0; sy < p.ssaa; ++sy) {
        for (int sx = 0; sx < p.ssaa; ++sx) {
            double ox = (x + (sx + 0.5) * inv - 0.5 * p.width) * p.spacing;
            double oy = (0.5 * p.height - (y + (sy + 0.5) * inv)) * p.spacing * p.flip_y;
            double dr = ox * p.cos_rot - oy * p.sin_rot, di = ox * p.sin_rot + oy * p.cos_rot;
            Sample s;
            if (p.fractal == MANDELBROT && p.spacing > CARDIOID_MIN_SPACING &&
                in_cardioid(p.centre_re + dr, p.centre_im + di))
                s = interior();
            else if (p.ref_len > 0 && p.fp32 && p.spacing < F32_MIN_SPACING)
                s = mandel_scaled_f32(p, ref32, dr, di);
            else if (p.ref_len > 0 && p.fp32)
                s = pixel_perturb(p, ref32, (float)dr, (float)di);
            else if (p.ref_len > 0)
                s = pixel_perturb(p, ref, dr, di);
            else
                s = pixel_f64(p, p.centre_re + dr, p.centre_im + di);
            colour(p, pal, s, acc);
        }
    }
    const float k = 255.0f / (p.ssaa * p.ssaa);
    unsigned char* px = rgb + 3 * ((size_t)y * p.width + x);
    for (int c = 0; c < 3; ++c) px[c] = (unsigned char)fminf(acc[c] * k + 0.5f, 255.0f);
}
