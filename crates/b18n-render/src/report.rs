//! Reporte JSON del render: trazabilidad de modelo, configuración y métricas.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

/// Versión del algoritmo de render; cambiar al variar el DSP de forma audible.
pub const ALGORITHM_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "-nonlinear-triodes-power-7199-inverter-nfb-25k-tpt-rate-2x-tone-mna-preamp-loading-7199-coupled-speaker-v1"
);

#[derive(Debug, Serialize)]
pub struct RenderReport {
    pub tool: &'static str,
    pub algorithm_version: &'static str,
    pub timestamp: Timestamp,
    pub model: ModelReport,
    pub input: AudioFileReport,
    pub output: AudioFileReport,
    pub settings: SettingsReport,
    pub metrics: MetricsReport,
    pub fidelity: String,
    pub latency_samples: u32,
    pub tone_measurement: Option<ToneMeasurementReport>,
}

/// Resultado del barrido de la red de tono (`--measure tone`).
#[derive(Debug, Serialize)]
pub struct ToneMeasurementReport {
    pub csv: String,
    pub combinations: usize,
    pub frequencies_per_curve: usize,
    pub bass_boost_db: f64,
    pub bass_cut_db: f64,
    pub treble_boost_db: f64,
    pub treble_cut_db: f64,
}

#[derive(Debug, Serialize)]
pub struct Timestamp {
    pub unix_epoch_s: u64,
    pub utc: String,
}

#[derive(Debug, Serialize)]
pub struct ModelReport {
    pub source: String,
    /// SHA-256 del JSON histórico de entrada (canónico o del usuario).
    pub sha256: String,
    /// SHA-256 de la errata aplicada; `None` si el modelo de entrada no es el
    /// canónico exacto al que la errata está enlazada.
    pub errata_sha256: Option<String>,
    /// SHA-256 del JSON efectivo tras aplicar la errata.
    pub effective_sha256: String,
    pub schematic_revision: String,
}

#[derive(Debug, Serialize)]
pub struct AudioFileReport {
    pub path: String,
    pub sample_rate_hz: u32,
    pub channels: usize,
    pub frames: usize,
}

#[derive(Debug, Serialize)]
pub struct SettingsReport {
    pub channel: &'static str,
    pub oversampling: u8,
    pub cabinet: &'static str,
    pub speaker: &'static str,
}

#[derive(Debug, Serialize)]
pub struct MetricsReport {
    pub peak: f32,
    pub rms: f32,
    pub dc_offset: f32,
}

impl RenderReport {
    /// Serializa con formato legible para revisión humana.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// Métricas básicas de un conjunto de canales.
#[must_use]
pub fn metrics(channels: &[Vec<f32>]) -> MetricsReport {
    let mut peak = 0.0_f32;
    let mut sum_squares = 0.0_f64;
    let mut sum = 0.0_f64;
    let mut count: u64 = 0;
    for channel in channels {
        for &sample in channel {
            peak = peak.max(sample.abs());
            sum_squares += f64::from(sample) * f64::from(sample);
            sum += f64::from(sample);
            count += 1;
        }
    }
    let (rms, dc_offset) = if count == 0 {
        (0.0, 0.0)
    } else {
        (
            (sum_squares / count as f64).sqrt() as f32,
            (sum / count as f64) as f32,
        )
    };
    MetricsReport {
        peak,
        rms,
        dc_offset,
    }
}

/// Timestamp actual en UTC sin dependencias externas.
#[must_use]
pub fn now_utc() -> Timestamp {
    let unix_epoch_s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    Timestamp {
        unix_epoch_s,
        utc: utc_from_unix(unix_epoch_s),
    }
}

/// Convierte segundos Unix a `YYYY-MM-DDThh:mm:ssZ` (algoritmo civil).
#[must_use]
pub fn utc_from_unix(epoch_s: u64) -> String {
    let days = (epoch_s / 86_400) as i64;
    let seconds_of_day = epoch_s % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds_of_day / 3_600,
        (seconds_of_day % 3_600) / 60,
        seconds_of_day % 60
    )
}

/// Días desde 1970-01-01 a fecha civil (Howard Hinnant, dominio público).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_zero_is_1970() {
        assert_eq!(utc_from_unix(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn known_date_matches() {
        assert_eq!(utc_from_unix(1_791_417_600), "2026-10-08T00:00:00Z");
    }

    #[test]
    fn metrics_of_silence_are_zero() {
        let result = metrics(&[vec![0.0; 16], vec![0.0; 16]]);
        assert_eq!(result.peak, 0.0);
        assert_eq!(result.rms, 0.0);
        assert_eq!(result.dc_offset, 0.0);
    }

    #[test]
    fn metrics_of_dc_signal() {
        let result = metrics(&[vec![0.5; 4]]);
        assert_eq!(result.peak, 0.5);
        assert!((result.rms - 0.5).abs() < 1e-6);
        assert!((result.dc_offset - 0.5).abs() < 1e-6);
    }
}
