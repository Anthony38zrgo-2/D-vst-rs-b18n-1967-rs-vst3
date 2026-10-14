//! Mandos de usuario del motor (`RuntimeControls`).
//!
//! Estos valores no modifican [`crate::circuit::CircuitDefinition`]: solo
//! indican posiciones de potenciómetros y ajustes modernos de adaptación de
//! nivel. Las posiciones se expresan como fracción `0.0..=1.0` del recorrido.

/// Entrada seleccionada según los conectores documentados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputChannel {
    #[default]
    Bass,
    Guitar,
    Instrument,
}

/// Modern approximate acoustic coloration, separate from the historical load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpeakerProfile {
    #[default]
    Direct,
    Eighteen,
    Fifteen,
}

/// Límites de los ajustes de nivel modernos (no históricos).
pub const TRIM_MIN_DB: f32 = -60.0;
pub const TRIM_MAX_DB: f32 = 24.0;

/// Posición de los mandos del amplificador y trims modernos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeControls {
    /// Adaptación moderna DAW↔voltios.
    pub input_trim_db: f32,
    pub channel: InputChannel,
    /// Canal 1: 0.0..=1.0 de recorrido del potenciómetro de volumen (1 MΩ log).
    pub volume_1: f32,
    /// Canal 1: potenciómetro de graves (1 MΩ lineal).
    pub bass_1: f32,
    /// Canal 1: potenciómetro de agudos (1 MΩ lineal).
    pub treble_1: f32,
    /// Canal 2: potenciómetro de volumen (1 MΩ log).
    pub volume_2: f32,
    /// Canal 2: potenciómetro de graves (1 MΩ lineal).
    pub bass_2: f32,
    /// Canal 2: potenciómetro de agudos (1 MΩ lineal).
    pub treble_2: f32,
    /// Normalización moderna de salida.
    pub output_trim_db: f32,
    /// IR de gabinete opcional (moderna).
    pub cabinet_enabled: bool,
    pub speaker_profile: SpeakerProfile,
}

impl Default for RuntimeControls {
    fn default() -> Self {
        Self {
            input_trim_db: 0.0,
            channel: InputChannel::default(),
            volume_1: 0.5,
            bass_1: 0.5,
            treble_1: 0.5,
            volume_2: 0.5,
            bass_2: 0.5,
            treble_2: 0.5,
            output_trim_db: 0.0,
            cabinet_enabled: false,
            speaker_profile: SpeakerProfile::Direct,
        }
    }
}

/// Fracción de tensión de un potenciómetro de volumen con taper de audio.
///
/// Aproximación documentada y asumida (véase `docs/MODEL_ASSUMPTIONS.md`):
/// 10 % de resistencia a mitad de recorrido, `f(p) = (10^(2p) − 1)/99`.
/// Sustituible por una curva medida del potenciómetro sin cambiar la API.
#[must_use]
pub fn volume_fraction(position: f32) -> f64 {
    let p = f64::from(position.clamp(0.0, 1.0));
    (10.0_f64.powf(2.0 * p) - 1.0) / 99.0
}

impl RuntimeControls {
    /// Restringe posiciones y trims a sus rangos válidos.
    #[must_use]
    pub fn sanitized(self) -> Self {
        let clamp_position = |value: f32| value.clamp(0.0, 1.0);
        let clamp_trim = |value: f32| {
            if value.is_finite() {
                value.clamp(TRIM_MIN_DB, TRIM_MAX_DB)
            } else {
                0.0
            }
        };
        Self {
            input_trim_db: clamp_trim(self.input_trim_db),
            channel: self.channel,
            volume_1: clamp_position(self.volume_1),
            bass_1: clamp_position(self.bass_1),
            treble_1: clamp_position(self.treble_1),
            volume_2: clamp_position(self.volume_2),
            bass_2: clamp_position(self.bass_2),
            treble_2: clamp_position(self.treble_2),
            output_trim_db: clamp_trim(self.output_trim_db),
            cabinet_enabled: self.cabinet_enabled,
            speaker_profile: self.speaker_profile,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_taper_hits_documented_anchors() {
        assert!((volume_fraction(0.0) - 0.0).abs() < 1e-12);
        assert!((volume_fraction(1.0) - 1.0).abs() < 1e-12);
        assert!((volume_fraction(0.5) - 0.0909).abs() < 0.001);
    }

    #[test]
    fn sanitize_clamps_positions_and_trims() {
        let controls = RuntimeControls {
            input_trim_db: 200.0,
            volume_1: 1.5,
            treble_1: -0.2,
            output_trim_db: f32::NAN,
            ..RuntimeControls::default()
        }
        .sanitized();

        assert_eq!(controls.input_trim_db, TRIM_MAX_DB);
        assert_eq!(controls.volume_1, 1.0);
        assert_eq!(controls.treble_1, 0.0);
        assert_eq!(controls.output_trim_db, 0.0);
    }
}
