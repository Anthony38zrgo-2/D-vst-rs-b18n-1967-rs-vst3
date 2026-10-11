//! Barrido de la red de tono y exportación de curvas.
//!
//! Usa la rejilla ISO de tercios de octava (20 Hz–20 kHz) y las condiciones
//! documentadas en `assets/fixtures/reference/tone_stack/README.md` para que
//! el CSV generado sea directamente comparable con la referencia numpy/SPICE.

use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

use b18n_dsp::{ToneStackControls, ToneStackDesign};

/// Posiciones de mando del protocolo (0/25/50/75/100 %).
pub const POSITIONS: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

/// Rejilla ISO de tercios de octava de 20 Hz a 20 kHz.
pub const FREQUENCIES_HZ: [f64; 31] = [
    20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0,
    500.0, 630.0, 800.0, 1_000.0, 1_250.0, 1_600.0, 2_000.0, 2_500.0, 3_150.0, 4_000.0, 5_000.0,
    6_300.0, 8_000.0, 10_000.0, 12_500.0, 16_000.0, 20_000.0,
];

/// Impedancia de la etapa que excita la red (salida del segundo 6SL7).
pub const SOURCE_OHMS: f64 = 100_000.0;

/// Carga de la red (resistor de mezcla de 270 kΩ).
pub const LOAD_OHMS: f64 = 270_000.0;

/// Resumen de la medición, para el reporte JSON.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToneCurveSummary {
    pub points: usize,
    pub bass_boost_db: f64,
    pub bass_cut_db: f64,
    pub treble_boost_db: f64,
    pub treble_cut_db: f64,
}

/// Error del barrido.
#[derive(Debug)]
pub enum SweepError {
    Io(io::Error),
    Singular {
        bass: f32,
        treble: f32,
        frequency_hz: f64,
    },
}

impl fmt::Display for SweepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(source) => write!(f, "error de E/S en el barrido: {source}"),
            Self::Singular {
                bass,
                treble,
                frequency_hz,
            } => write!(
                f,
                "red singular en bass={bass} treble={treble} f={frequency_hz} Hz"
            ),
        }
    }
}

impl std::error::Error for SweepError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::Singular { .. } => None,
        }
    }
}

impl From<io::Error> for SweepError {
    fn from(source: io::Error) -> Self {
        Self::Io(source)
    }
}

fn magnitude_db(
    design: &ToneStackDesign,
    bass: f32,
    treble: f32,
    frequency_hz: f64,
) -> Result<f64, SweepError> {
    design
        .frequency_response(
            ToneStackControls { bass, treble },
            frequency_hz,
            SOURCE_OHMS,
            LOAD_OHMS,
        )
        .map(|response| response.magnitude_db())
        .ok_or(SweepError::Singular {
            bass,
            treble,
            frequency_hz,
        })
}

/// Escribe las 25 combinaciones a `path` y devuelve el resumen de rangos.
pub fn write_tone_curves(
    path: &Path,
    design: &ToneStackDesign,
) -> Result<ToneCurveSummary, SweepError> {
    let mut csv = String::with_capacity(64 * 1024);
    csv.push_str("bass,treble,freq_hz,mag_db,phase_deg\n");

    let mut points = 0_usize;
    for bass in POSITIONS {
        for treble in POSITIONS {
            for frequency in FREQUENCIES_HZ {
                let response = design
                    .frequency_response(
                        ToneStackControls { bass, treble },
                        frequency,
                        SOURCE_OHMS,
                        LOAD_OHMS,
                    )
                    .ok_or(SweepError::Singular {
                        bass,
                        treble,
                        frequency_hz: frequency,
                    })?;
                writeln!(
                    csv,
                    "{bass:.2},{treble:.2},{frequency:.1},{:.9},{:.6}",
                    response.magnitude_db(),
                    response.phase_deg()
                )
                .expect("escritura en String infalible");
                points += 1;
            }
        }
    }

    fs::write(path, csv)?;

    Ok(ToneCurveSummary {
        points,
        bass_boost_db: magnitude_db(design, 1.0, 0.5, 40.0)?
            - magnitude_db(design, 0.5, 0.5, 40.0)?,
        bass_cut_db: magnitude_db(design, 0.5, 0.5, 40.0)? - magnitude_db(design, 0.0, 0.5, 40.0)?,
        treble_boost_db: magnitude_db(design, 0.5, 1.0, 5_000.0)?
            - magnitude_db(design, 0.5, 0.5, 5_000.0)?,
        treble_cut_db: magnitude_db(design, 0.5, 0.5, 5_000.0)?
            - magnitude_db(design, 0.5, 0.0, 5_000.0)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use b18n_dsp::ToneStackDesign;

    #[test]
    fn writes_full_grid_with_documented_ranges() {
        let circuit = b18n_config::load_reference().expect("JSON canónico válido");
        let design = ToneStackDesign::from_circuit(&circuit).expect("red completa");
        let path =
            std::env::temp_dir().join(format!("b18n_sweep_{}_tone_curves.csv", std::process::id()));

        let summary = write_tone_curves(&path, &design).expect("barrido");
        let contents = std::fs::read_to_string(&path).expect("lectura");
        std::fs::remove_file(&path).ok();

        assert_eq!(summary.points, 25 * 31);
        assert_eq!(contents.lines().count(), 25 * 31 + 1);
        assert!((4.0..15.0).contains(&summary.bass_boost_db));
        assert!((4.0..25.0).contains(&summary.bass_cut_db));
        assert!((5.0..20.0).contains(&summary.treble_boost_db));
        assert!((15.0..40.0).contains(&summary.treble_cut_db));
    }
}
