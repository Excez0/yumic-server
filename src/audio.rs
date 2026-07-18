use anyhow::{Context, Result};
use std::io::Write;
use std::process::Command;
use tracing::{debug, info, warn};

const PIPE_PATH: &str = "/tmp/yumic_audio_pipe";

/// PulseAudio module manager - creates a virtual source from a FIFO pipe.
pub struct PulseAudioModules {
    source_name: String,
    module_id: Option<u32>,
}

impl PulseAudioModules {
    pub fn new(_sink_name: &str, source_name: &str) -> Self {
        Self {
            source_name: source_name.to_string(),
            module_id: None,
        }
    }

    pub fn setup(&mut self) -> Result<()> {
        self.cleanup_stale();

        let _ = std::fs::remove_file(PIPE_PATH);
        std::process::Command::new("mkfifo")
            .arg(PIPE_PATH)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to create FIFO: {}", e))?;
        info!("Created FIFO at {}", PIPE_PATH);

        let output = Command::new("pactl")
            .args([
                "load-module",
                "module-pipe-source",
                &format!("file={}", PIPE_PATH),
                &format!("source_name={}", self.source_name),
                "channels=1",
                "rate=48000",
                "format=s16le",
                &format!("source_properties=device.description=YuMic_Microphone"),
            ])
            .output()
            .context("Failed to execute pactl")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create pipe source: {}", stderr.trim());
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Ok(id) = stdout.parse::<u32>() {
            self.module_id = Some(id);
            info!("PulseAudio pipe-source module loaded (ID: {})", id);
        }

        // Give PulseAudio time to open the read end of the FIFO
        std::thread::sleep(std::time::Duration::from_millis(500));
        info!("Pipe source '{}' is ready.", self.source_name);
        Ok(())
    }

    fn cleanup_stale(&self) {
        let output = Command::new("pactl")
            .args(["list", "modules", "short"])
            .output();
        if let Ok(o) = output {
            let stdout = String::from_utf8_lossy(&o.stdout);
            for line in stdout.lines() {
                if line.contains("module-pipe-source") && line.contains(&self.source_name) {
                    if let Some(id_str) = line.split_whitespace().next() {
                        if let Ok(id) = id_str.parse::<u32>() {
                            info!("Unloading stale module {}...", id);
                            let _ = Command::new("pactl")
                                .args(["unload-module", &id.to_string()])
                                .output();
                        }
                    }
                }
            }
        }
    }

    pub fn cleanup(&mut self) -> Result<()> {
        if let Some(id) = self.module_id.take() {
            info!("Unloading pipe-source module (ID: {})...", id);
            let output = Command::new("pactl")
                .args(["unload-module", &id.to_string()])
                .output()
                .context("Failed to unload module")?;
            if !output.status.success() {
                warn!(
                    "Failed to unload module {}: {}",
                    id,
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
        }
        info!("PulseAudio cleanup complete");
        Ok(())
    }
}

impl Drop for PulseAudioModules {
    fn drop(&mut self) {
        if self.module_id.is_some() {
            if let Err(e) = self.cleanup() {
                warn!("Error during cleanup: {:?}", e);
            }
        }
    }
}

/// Pipe writer — writes PCM16LE to a named FIFO using non-blocking I/O.
/// If the pipe buffer is full (PulseAudio can't keep up), data is dropped
/// rather than blocking the server.
pub struct PipeWriter {
    file: Option<std::fs::File>,
    path: String,
}

impl PipeWriter {
    pub fn new(path: &str) -> Self {
        Self {
            file: None,
            path: path.to_string(),
        }
    }

    pub fn open(&mut self) -> Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        info!("Opening pipe '{}' for writing (non-blocking)...", self.path);
        let file = std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&self.path)
            .with_context(|| format!("Failed to open pipe '{}'", self.path))?;
        self.file = Some(file);
        info!("Pipe opened (48kHz S16LE mono)");
        Ok(())
    }

    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        use std::io::ErrorKind;
        if let Some(ref mut file) = self.file {
            match file.write_all(data) {
                Ok(()) => {
                    file.flush().ok();
                    Ok(())
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    // Pipe buffer full — drop this chunk, PulseAudio will catch up
                    debug!("Pipe full, dropping {} bytes", data.len());
                    Ok(())
                }
                Err(e) => Err(e).context("Pipe write failed"),
            }
        } else {
            anyhow::bail!("Pipe not opened");
        }
    }

    pub fn close(&mut self) {
        self.file = None;
    }
}

/// Opus decoder
pub struct OpusDecoder {
    decoder: opus::Decoder,
}

impl OpusDecoder {
    pub fn new() -> Result<Self> {
        let decoder = opus::Decoder::new(48000, opus::Channels::Mono)
            .context("Failed to create Opus decoder")?;
        info!("Opus decoder created (48kHz, mono)");
        Ok(Self { decoder })
    }

    pub fn decode(&mut self, opus_data: &[u8]) -> Result<Vec<u8>> {
        // 5760 = 48kHz * 120ms max frame size (Opus max frame size at 48kHz)
        // Opus max frame size at 48kHz = 120ms = 5760 samples
        let mut pcm = vec![0i16; 5760];
        let decoded_samples = self
            .decoder
            .decode(opus_data, &mut pcm, false)
            .context("Opus decode failed")?;

        let byte_len = decoded_samples * 2;
        let mut bytes = Vec::with_capacity(byte_len);
        for &sample in &pcm[..decoded_samples] {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        debug!(
            "Decoded Opus: {}B -> {} samples ({} bytes)",
            opus_data.len(),
            decoded_samples,
            byte_len
        );
        Ok(bytes)
    }
}
