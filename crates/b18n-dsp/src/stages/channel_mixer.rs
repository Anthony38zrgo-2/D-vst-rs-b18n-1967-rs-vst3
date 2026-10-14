//! Mezcla de canales: dos resistores de **270 kΩ** hacia un nodo común que es
//! la rejilla del pentodo 7199 (pin 7), **acoplada en continua** (B18-006).
//!
//! # Trazado (SCH-567 + tabla de resistencias del manual)
//!
//! Las salidas de ambos tonos se suman a través de 270 kΩ por canal
//! (`channel_mixer_resistor_ohms`) en un nodo compartido que va **directo** a
//! la rejilla del pentodo. No hay capacitor de 0.05 µF ni fuga de 1 MΩ en ese
//! nodo: los 0.05 µF del esquema son los acoplos del inversor a las 7027A.
//! Evidencia: la tabla `RESISTANCE READINGS` del manual mide en V3/pin 7
//! **220 kΩ con Bass c.c.w.** y **330 kΩ con Bass c.w.**, valores que salen de
//! `(270 kΩ + Z_tono) / 2` con las dos ramas en paralelo (véase el test
//! `mixer_node_resistance_matches_manual_reading`).
//!
//! Con fuentes de tono idealizadas, el nodo vale `(v1 + v2) / 2`: el canal
//! inactivo aporta su 270 kΩ a masa como carga real. La impedancia de salida
//! de cada red de tono en serie con su 270 kΩ queda pendiente del nodo acoplado
//! del inversor (B18-008), que resolverá la rejilla del pentodo con su red.

/// Mezclador pasivo de dos canales (nodo compartido resistivo).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelMixer {
    mixer_ohms: f64,
    grid_load_ohms: Option<f64>,
}

impl ChannelMixer {
    /// `mixer_ohms` = resistor de mezcla por canal (270 kΩ en el JSON).
    #[must_use]
    pub fn new(mixer_ohms: f64) -> Self {
        Self {
            mixer_ohms,
            grid_load_ohms: None,
        }
    }

    /// Variante con una carga explícita de rejilla (no trazada en 1967; se
    /// conserva para ensayos de sensibilidad).
    #[must_use]
    pub fn with_grid_load(mixer_ohms: f64, grid_load_ohms: Option<f64>) -> Self {
        Self {
            mixer_ohms,
            grid_load_ohms,
        }
    }

    /// Ganancia del nodo para fuentes ideales: `g / (2g + G_rejilla)`.
    #[must_use]
    pub fn node_gain(&self) -> f64 {
        let g = 1.0 / self.mixer_ohms;
        let load = self.grid_load_ohms.map_or(0.0, |ohms| 1.0 / ohms);
        g / (2.0 * g + load)
    }

    #[must_use]
    pub fn mixer_ohms(&self) -> f64 {
        self.mixer_ohms
    }

    /// Mezcla las tensiones de salida de tono de ambos canales.
    #[must_use]
    pub fn process(&self, channel_1_volts: f64, channel_2_volts: f64) -> f64 {
        self.node_gain() * (channel_1_volts + channel_2_volts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::test_circuit;
    use crate::stages::tone_stack::{ToneStackControls, ToneStackDesign};

    #[test]
    fn ideal_sources_sum_with_half_gain() {
        let mixer = ChannelMixer::new(270_000.0);
        assert!((mixer.node_gain() - 0.5).abs() < 1e-12);
        // Ambos canales iguales: el nodo vale la señal completa.
        assert!((mixer.process(1.0, 1.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn idle_channel_loads_the_node() {
        let mixer = ChannelMixer::new(270_000.0);
        let both = mixer.process(1.0, 1.0);
        let single = mixer.process(1.0, 0.0);
        // El canal inactivo deja su 270 kΩ a masa: mitad exacta.
        assert!(
            (both - 2.0 * single).abs() < 1e-12,
            "both {both} single {single}"
        );
    }

    #[test]
    fn explicit_grid_load_reduces_the_node() {
        let with_load = ChannelMixer::with_grid_load(270_000.0, Some(1_000_000.0));
        let expected = (1.0 / 270_000.0) / (2.0 / 270_000.0 + 1.0 / 1_000_000.0);
        assert!((with_load.node_gain() - expected).abs() < 1e-12);
        assert!(with_load.node_gain() < 0.5);
    }

    /// Tabla del manual (V3/pin 7): 220 kΩ con Bass c.c.w. y 330 kΩ con Bass
    /// c.w. midiendo desde la rejilla a masa a través de los dos 270 kΩ y las
    /// redes de tono (treble en el extremo de r_t_out≈0).
    #[test]
    fn mixer_node_resistance_matches_manual_reading() {
        let design = ToneStackDesign::from_circuit(&test_circuit()).expect("red de tono");
        let reading = |bass: f32| {
            let z = design.dc_output_resistance(ToneStackControls { bass, treble: 1.0 });
            (270_000.0 + z) / 2.0
        };
        let bass_cut = reading(0.0);
        let bass_boost = reading(1.0);
        if std::env::var_os("B18N_MIXER_REPORT").is_some() {
            println!(
                "V3/pin 7 (manual 220k/330k): Bass c.c.w. {bass_cut:.0} Ω; Bass c.w. {bass_boost:.0} Ω"
            );
        }
        assert!(
            (bass_cut - 220_000.0).abs() / 220_000.0 < 0.10,
            "Bass c.c.w.: {bass_cut:.0} Ω"
        );
        assert!(
            (bass_boost - 330_000.0).abs() / 330_000.0 < 0.10,
            "Bass c.w.: {bass_boost:.0} Ω"
        );
        assert!(bass_boost > bass_cut);
    }
}
