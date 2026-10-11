//! Entrada: adaptación de muestra digital a tensión física.
//!
//! Convención documentada en `docs/MODEL_ASSUMPTIONS.md`: una muestra de
//! amplitud `1.0` equivale a `1.0 V` de pico antes del ajuste de entrada
//! (`InputTrimDb`, añadido moderno). La carga de la red de entrada
//! (120 kΩ / 5.6 MΩ) no altera el nivel de forma significativa frente a la
//! impedancia de salida de un equipo de línea y se incorporará con la
//! topología de entrada completa en el Hito D.

use crate::utilities::db::db_to_gain;

/// Tensión de pico correspondiente a amplitud digital `1.0`.
pub const VPK_PER_FULL_SCALE: f64 = 1.0;

/// Adapta muestras a tensión y aplica el trim de entrada.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputStage {
    trim_db: f32,
    gain: f64,
}

impl Default for InputStage {
    fn default() -> Self {
        Self {
            trim_db: 0.0,
            gain: 1.0,
        }
    }
}

impl InputStage {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_trim_db(&mut self, trim_db: f32) {
        self.trim_db = trim_db;
        self.gain = f64::from(db_to_gain(trim_db));
    }

    #[must_use]
    pub fn trim_db(&self) -> f32 {
        self.trim_db
    }

    /// Convierte una muestra a tensión de trabajo.
    #[must_use]
    pub fn process_sample(&self, sample: f32) -> f64 {
        f64::from(sample) * VPK_PER_FULL_SCALE * self.gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unity_without_trim() {
        let stage = InputStage::new();
        assert_eq!(stage.process_sample(0.5), 0.5);
    }

    #[test]
    fn trim_scales_level() {
        let mut stage = InputStage::new();
        stage.set_trim_db(-6.0206);
        let output = stage.process_sample(1.0);
        assert!((output - 0.5).abs() < 1e-4, "salida {output}");
    }
}
