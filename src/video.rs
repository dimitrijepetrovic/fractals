use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};

use anyhow::Context;

/// Pipes raw RGB frames into an ffmpeg child process.
pub struct VideoWriter {
    child: Child,
    stdin: ChildStdin,
}

impl VideoWriter {
    /// A video at `path`, encoded with `encoder` (libx264/libx265 at quality `crf`).
    pub fn video(path: &str, width: i32, height: i32, fps: f64, encoder: &str, crf: u32) -> anyhow::Result<Self> {
        let mut cmd = ffmpeg(width, height, fps);
        cmd.args(["-c:v", encoder, "-pix_fmt", "yuv420p"]);
        match encoder {
            "libx264" | "libx265" => cmd.args(["-preset", "slow", "-crf", &crf.to_string()]),
            "ffv1" => cmd.args(["-level", "3"]),
            _ => cmd.args(["-b:v", "60M"]),
        };
        if path.ends_with(".mp4") || path.ends_with(".mov") {
            cmd.args(["-movflags", "+faststart"]);
        }
        Self::spawn(cmd.arg(path))
    }

    /// A single image (format from the extension, e.g. .png).
    pub fn image(path: &str, width: i32, height: i32) -> anyhow::Result<Self> {
        let mut cmd = ffmpeg(width, height, 1.0);
        Self::spawn(cmd.args(["-frames:v", "1", "-update", "1"]).arg(path))
    }

    fn spawn(cmd: &mut Command) -> anyhow::Result<Self> {
        let mut child = cmd.stdin(Stdio::piped()).spawn().context("could not start ffmpeg (is it on PATH?)")?;
        let stdin = child.stdin.take().unwrap();
        Ok(Self { child, stdin })
    }

    pub fn write(&mut self, rgb: &[u8]) -> anyhow::Result<()> {
        self.stdin.write_all(rgb).context("writing to ffmpeg failed")
    }

    pub fn finish(self) -> anyhow::Result<()> {
        let Self { mut child, stdin } = self;
        drop(stdin);
        let status = child.wait()?;
        anyhow::ensure!(status.success(), "ffmpeg exited with an error");
        Ok(())
    }
}

fn ffmpeg(width: i32, height: i32, fps: f64) -> Command {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-hide_banner", "-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgb24"])
        .args(["-s", &format!("{width}x{height}"), "-framerate", &fps.to_string(), "-i", "-"]);
    cmd
}
