//! Entrada: adaptación de muestra digital a tensión física y red de jacks.
//!
//! Convención documentada en `docs/MODEL_ASSUMPTIONS.md`: una muestra de
//! amplitud `1.0` equivale a `1.0 V` de pico antes del ajuste de entrada
//! (`InputTrimDb`, añadido moderno).
//!
//! # Red de jacks (B18-007, trazada del raster SCH-567)
//!
//! - **CH1 (`bass` / `guitar`):** cada jack lleva **120 kΩ** en serie hacia la
//!   rejilla (pin 4 de V1) y la fuga de rejilla es **5.6 MΩ**. El jack
//!   `guitar` (inferior) es conmutado: su llave, cerrada sin plug, corta su
//!   punta a masa a través de **100 kΩ**. Por eso, con el jack `bass` en uso y
//!   el `guitar` libre, la carga efectiva de rejilla es
//!   `5.6 MΩ ∥ (120 kΩ + 100 kΩ) = 211.8 kΩ` (el manual mide **220 kΩ** en
//!   V1/pin 4, dentro del ±10 % de tolerancia). Con el jack `guitar` en uso su
//!   llave abre y la carga es la fuga sola.
//! - **CH2 (`instrument`):** un solo jack conmutado, **sin** resistencia serie
//!   (la punta va directa a la rejilla de V2/pin 4) y fuga de **2.2 MΩ**. Sin
//!   plug, la llave corta la punta a masa (apantallado); el manual lista
//!   220 kΩ en V2/pin 4, valor que coincide con la fila de V1 y que el raster
//!   no respalda (se registra la discrepancia; manda la conexión trazada).
//!
//! No se modela una curva «bass boost» ni «guitar boost»: la diferencia entre
//! jacks es la carga conmutada descrita, no una ecualización.

use crate::controls::InputChannel;
use crate::utilities::db::db_to_gain;

/// Tensión de pico correspondiente a amplitud digital `1.0`.
pub const VPK_PER_FULL_SCALE: f64 = 1.0;

/// Red física de un jack de entrada (Thevenin visto por la rejilla del 6SL7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputJackNetwork {
    /// Resistencia serie entre la punta del jack y la rejilla (Ω).
    pub series_ohms: f64,
    /// Carga efectiva de rejilla a masa con ese jack en uso (Ω).
    pub grid_load_ohms: f64,
}

impl InputJackNetwork {
    /// Red trazada por posición de entrada (B18-007).
    #[must_use]
    pub fn for_channel(channel: InputChannel) -> Self {
        match channel {
            // Jack `bass` (superior, sin llave): el `guitar` libre corta su
            // punta a masa por 100 kΩ tras su 120 kΩ serie.
            InputChannel::Bass => Self {
                series_ohms: 120_000.0,
                grid_load_ohms: parallel(5_600_000.0, 220_000.0),
            },
            // Jack `guitar` (inferior, conmutado): al insertar el plug la
            // llave abre el shunt de 100 kΩ; el `bass` libre no carga.
            InputChannel::Guitar => Self {
                series_ohms: 120_000.0,
                grid_load_ohms: 5_600_000.0,
            },
            // Jack `instrument` (CH2, conmutado): punta directa a la rejilla.
            InputChannel::Instrument => Self {
                series_ohms: 0.0,
                grid_load_ohms: 2_200_000.0,
            },
        }
    }

    /// Fracción de la tensión de fuente que llega a la rejilla para una fuente
    /// de impedancia `source_ohms` (0 en el modelo de la DAW).
    #[must_use]
    pub fn grid_division(&self, source_ohms: f64) -> f64 {
        self.grid_load_ohms / (source_ohms + self.series_ohms + self.grid_load_ohms)
    }
}

fn parallel(a: f64, b: f64) -> f64 {
    a * b / (a + b)
}

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

    /// V1/pin 4 del manual: 220 kΩ con los jacks libres (±10 %).
    #[test]
    fn bass_jack_load_matches_manual_reading() {
        let network = InputJackNetwork::for_channel(InputChannel::Bass);
        let reading = network.grid_load_ohms;
        assert!(
            (reading - 220_000.0).abs() / 220_000.0 < 0.10,
            "carga trazada {reading} Ω fuera del ±10 % de 220 kΩ"
        );
    }

    #[test]
    fn guitar_and_instrument_networks_follow_the_trace() {
        let guitar = InputJackNetwork::for_channel(InputChannel::Guitar);
        assert_eq!(guitar.series_ohms, 120_000.0);
        assert_eq!(guitar.grid_load_ohms, 5_600_000.0);

        let instrument = InputJackNetwork::for_channel(InputChannel::Instrument);
        assert_eq!(instrument.series_ohms, 0.0);
        assert_eq!(instrument.grid_load_ohms, 2_200_000.0);
    }

    #[test]
    fn guitar_is_hotter_than_bass_and_instrument_is_direct() {
        let division = |channel| InputJackNetwork::for_channel(channel).grid_division(0.0);
        let bass = division(InputChannel::Bass);
        let guitar = division(InputChannel::Guitar);
        let instrument = division(InputChannel::Instrument);
        assert!(bass < guitar && guitar < instrument);
        assert!((instrument - 1.0).abs() < 1e-12);
    }
}
