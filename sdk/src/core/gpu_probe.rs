//! NVIDIA GPU/VRAM probing (phase 2.6b — docs/local-stt-plan.md §2).
//!
//! First local-STT release is NVIDIA-only (see plan). `nvidia-smi` ships
//! with the driver on both Windows and Linux, so shelling out to it reports
//! the same [`HardwareInfo`] the benchmark report already carries, without a
//! native NVML binding. No NVIDIA GPU (missing binary, AMD/Intel-only, or
//! unparseable output) degrades to `HardwareInfo::default()` — the caller
//! (dashboard "hardware test") treats that as "requirement not met", not a
//! crash.

use std::process::Command;

use super::stt_control::HardwareInfo;

const QUERY_ARGS: &[&str] = &[
    "--query-gpu=name,memory.total",
    "--format=csv,noheader,nounits",
];

/// Probes the first NVIDIA GPU's name and total VRAM (MiB). `vram_mb` is
/// total VRAM, matching the server policy's `min_vram_mb` threshold
/// (`stt.local.min_vram_mb`, a static GPU-tier requirement) — free VRAM at
/// stream time is a separate runtime watchdog concern, not reported here.
pub fn probe() -> HardwareInfo {
    match Command::new("nvidia-smi").args(QUERY_ARGS).output() {
        Ok(output) if output.status.success() => String::from_utf8(output.stdout)
            .ok()
            .and_then(|text| parse_first_gpu(&text))
            .unwrap_or_default(),
        _ => HardwareInfo::default(),
    }
}

/// Parses `nvidia-smi --query-gpu=name,memory.total --format=csv,noheader,nounits`
/// output: one `name, total_mib` line per GPU. Only the first line is used —
/// multi-GPU selection is out of scope for the first release.
fn parse_first_gpu(text: &str) -> Option<HardwareInfo> {
    let line = text.lines().next()?.trim();
    let (name, total) = line.rsplit_once(',')?;
    let vram_mb: u64 = total.trim().parse().ok()?;
    let name = name.trim();
    if name.is_empty() || vram_mb == 0 {
        return None;
    }
    Some(HardwareInfo {
        gpu_name: Some(name.to_string()),
        vram_mb: Some(vram_mb),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_gpu_line() {
        let info =
            parse_first_gpu("NVIDIA GeForce RTX 3070 Laptop GPU, 8192\n").expect("parsed");
        assert_eq!(
            info.gpu_name.as_deref(),
            Some("NVIDIA GeForce RTX 3070 Laptop GPU")
        );
        assert_eq!(info.vram_mb, Some(8192));
    }

    #[test]
    fn picks_first_of_multiple_gpus() {
        let info = parse_first_gpu("RTX 3060, 12288\nRTX 2060, 6144\n").expect("parsed");
        assert_eq!(info.gpu_name.as_deref(), Some("RTX 3060"));
        assert_eq!(info.vram_mb, Some(12288));
    }

    #[test]
    fn trims_whitespace_around_fields() {
        let info = parse_first_gpu("  RTX 4090  ,   24576  ").expect("parsed");
        assert_eq!(info.gpu_name.as_deref(), Some("RTX 4090"));
        assert_eq!(info.vram_mb, Some(24576));
    }

    #[test]
    fn rejects_garbage_output() {
        assert!(parse_first_gpu("").is_none());
        assert!(parse_first_gpu("not csv at all").is_none());
        assert!(parse_first_gpu("Name, notanumber").is_none());
        assert!(parse_first_gpu("Name, 0").is_none());
        assert!(parse_first_gpu(", 8192").is_none());
    }

    /// Bu makinede gerçek `nvidia-smi` var (RTX 3070 Laptop) — mevcutsa
    /// gerçek isim/VRAM okunmalı; yoksa (CI/GPU'suz makine) default'a
    /// düşmeli, panik değil.
    #[test]
    fn real_probe_is_best_effort() {
        let has_nvidia_smi = Command::new("nvidia-smi")
            .arg("-L")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        let info = probe();
        if has_nvidia_smi {
            assert!(
                info.gpu_name.is_some() && info.vram_mb.is_some(),
                "nvidia-smi var ama probe() donanım okuyamadı: {info:?}"
            );
        } else {
            assert_eq!(info.gpu_name, None);
            assert_eq!(info.vram_mb, None);
        }
    }
}
