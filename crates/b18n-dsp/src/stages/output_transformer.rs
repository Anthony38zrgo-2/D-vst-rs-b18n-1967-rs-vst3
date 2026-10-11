//! Transformador de salida OT-471 (primera implementación: ideal con
//! resistencias serie y carga nominal).
//!
//! - Relación de transformación `N` (primario total / secundario 8 Ω).
//! - Pérdida por DCR primaria: `Zref/(Zref + Rdc_p)` con
//!   `Zref = Rcarga·N²`.
//! - Pérdida por DCR secundaria: `Rcarga/(Rcarga + Rdc_s)`.
//!
//! Inductancia de magnetización, fugas, saturación y carga reactiva quedan
//! como `None` en el JSON y no se inventan (Hito D, con evidencia).

/// Etapa del transformador de salida.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputTransformerStage {
    turns_ratio: f64,
    dc_primary_ohms: f64,
    dc_secondary_ohms: f64,
    load_ohms: f64,
}

impl OutputTransformerStage {
    #[must_use]
    pub fn new(
        turns_ratio: f64,
        dc_primary_ohms: f64,
        dc_secondary_ohms: f64,
        load_ohms: f64,
    ) -> Self {
        Self {
            turns_ratio,
            dc_primary_ohms,
            dc_secondary_ohms,
            load_ohms,
        }
    }

    #[must_use]
    pub fn turns_ratio(&self) -> f64 {
        self.turns_ratio
    }

    /// Convierte las excursiones diferenciales de placa en tensión de carga.
    #[must_use]
    pub fn process(&self, plate_positive_v: f64, plate_negative_v: f64) -> f64 {
        let primary = plate_positive_v - plate_negative_v;
        let reflected = self.load_ohms * self.turns_ratio * self.turns_ratio;
        let primary_effective = primary * reflected / (reflected + self.dc_primary_ohms);
        let secondary = primary_effective / self.turns_ratio;
        secondary * self.load_ohms / (self.load_ohms + self.dc_secondary_ohms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ideal_ratio_without_dcr() {
        let stage = OutputTransformerStage::new(23.18, 0.0, 0.0, 8.0);
        let output = stage.process(1.0, -1.0);
        assert!((output - 2.0 / 23.18).abs() < 1e-12);
    }

    #[test]
    fn dcr_produces_documented_losses() {
        let stage = OutputTransformerStage::new(23.18, 300.0, 0.4, 8.0);
        let output = stage.process(1.0, -1.0);
        let reflected = 8.0 * 23.18 * 23.18;
        let expected = 2.0 * (reflected / (reflected + 300.0)) / 23.18 * (8.0 / 8.4);
        assert!((output - expected).abs() < 1e-12);
        assert!(output < 2.0 / 23.18, "la DCR debe atenuar");
    }
}
