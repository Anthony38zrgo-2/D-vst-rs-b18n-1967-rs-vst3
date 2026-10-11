//! Contexto de audio del motor: sample rate, tamaño de bloque y sobremuestreo.

use crate::engine::EngineError;

/// Especificación de procesamiento negociada al inicializar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProcessSpec {
    pub sample_rate_hz: f64,
    /// Máximo de frames por bloque que el host puede entregar.
    pub max_block_frames: usize,
    /// Factor de sobremuestreo implementado: 1, 4 u 8.
    pub oversampling: u8,
}

impl ProcessSpec {
    /// Tasas que el proyecto acepta explícitamente (guía §6.3).
    pub const PREFERRED_SAMPLE_RATES_HZ: [f64; 5] =
        [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0];

    pub const SUPPORTED_OVERSAMPLING: [u8; 3] = [1, 4, 8];

    /// Valida los valores sin imponer topes no documentados.
    pub fn validate(&self) -> Result<(), EngineError> {
        if !self.sample_rate_hz.is_finite() || !(8_000.0..=384_000.0).contains(&self.sample_rate_hz)
        {
            return Err(EngineError::InvalidSampleRate {
                hz: self.sample_rate_hz,
            });
        }
        if self.max_block_frames == 0 {
            return Err(EngineError::InvalidBlockSize { frames: 0 });
        }
        if !Self::SUPPORTED_OVERSAMPLING.contains(&self.oversampling) {
            return Err(EngineError::UnsupportedOversampling {
                factor: self.oversampling,
            });
        }
        Ok(())
    }

    /// `true` si la tasa está entre las preferidas por el proyecto.
    #[must_use]
    pub fn is_preferred_sample_rate(&self) -> bool {
        Self::PREFERRED_SAMPLE_RATES_HZ.contains(&self.sample_rate_hz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_all_preferred_rates() {
        for hz in ProcessSpec::PREFERRED_SAMPLE_RATES_HZ {
            let spec = ProcessSpec {
                sample_rate_hz: hz,
                max_block_frames: 512,
                oversampling: 4,
            };
            assert!(spec.validate().is_ok(), "tasa {hz} rechazada");
        }
    }

    #[test]
    fn rejects_invalid_values() {
        let bad_rate = ProcessSpec {
            sample_rate_hz: f64::NAN,
            max_block_frames: 512,
            oversampling: 4,
        };
        assert!(matches!(
            bad_rate.validate(),
            Err(EngineError::InvalidSampleRate { .. })
        ));

        let bad_block = ProcessSpec {
            sample_rate_hz: 48_000.0,
            max_block_frames: 0,
            oversampling: 4,
        };
        assert!(matches!(
            bad_block.validate(),
            Err(EngineError::InvalidBlockSize { .. })
        ));

        let bad_oversampling = ProcessSpec {
            sample_rate_hz: 48_000.0,
            max_block_frames: 512,
            oversampling: 2,
        };
        assert!(matches!(
            bad_oversampling.validate(),
            Err(EngineError::UnsupportedOversampling { factor: 2 })
        ));
    }
}
