// With the `cuda` feature, compiles cuda/fractals.cu to PTX; the binary embeds it and the
// CUDA driver JIT-compiles it for the installed GPU at start-up.
use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=cuda/fractals.cu");
    println!("cargo:rerun-if-env-changed=CUDA_ARCH");
    if env::var_os("CARGO_FEATURE_CUDA").is_none() {
        return;
    }
    let ptx = PathBuf::from(env::var("OUT_DIR").unwrap()).join("fractals.ptx");
    let arch = env::var("CUDA_ARCH").unwrap_or_else(|_| "native".into());
    let status = Command::new("nvcc")
        .args(["-O3", "-ptx", &format!("-arch={arch}"), "-o"])
        .arg(&ptx)
        .arg("cuda/fractals.cu")
        .status()
        .expect("failed to run nvcc; install the CUDA toolkit or build with --no-default-features (CPU only)");
    assert!(status.success(), "nvcc failed to compile cuda/fractals.cu");
}
