//! Mezcla de canales: 270 kΩ por canal hacia un nodo común y acoplo de 0.05 µF
//! hacia la rejilla de la 7199.
//!
//! Modelo resistivo documentado: los dos resistores de mezcla son idénticos,
//! por lo que el nodo vale `g·(v1 + v2)` con
//! `g = (1/Rm) / (2/Rm + 1/Rg)`. La fuga de rejilla `Rg = 1 MΩ` es una
//! aproximación declarada en `docs/MODEL_ASSUMPTIONS.md` hasta digitalizar el
//! nodo exacto (Hito D desarrollará la etapa completa).

use std::f64::consts::PI;

use crate::utilities::filters::OnePoleHighPass;

/// Mezclador pasivo de dos canales con acoplo.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelMixer {
    node_gain: f64,
    coupling: OnePoleHighPass,
    mixer_ohms: f64,
    grid_leak_ohms: f64,
}

impl ChannelMixer {
    pub const DEFAULT_GRID_LEAK_OHMS: f64 = 1_000_000.0;

    /// `mixer_ohms` = resistor de mezcla por canal (270 kΩ en el JSON).
    #[must_use]
    pub fn new(mixer_ohms: f64, coupling_farads: f64, sample_rate_hz: f64) -> Self {
        Self::with_grid_leak(
            mixer_ohms,
            Self::DEFAULT_GRID_LEAK_OHMS,
            coupling_farads,
            sample_rate_hz,
        )
    }

    #[must_use]
    pub fn with_grid_leak(
        mixer_ohms: f64,
        grid_leak_ohms: f64,
        coupling_farads: f64,
        sample_rate_hz: f64,
    ) -> Self {
        let node_gain = (1.0 / mixer_ohms) / (2.0 / mixer_ohms + 1.0 / grid_leak_ohms);
        let parallel = 1.0 / (1.0 / mixer_ohms + 1.0 / mixer_ohms);
        let total = parallel + grid_leak_ohms;
        let cutoff_hz = 1.0 / (2.0 * PI * coupling_farads * total);
        Self {
            node_gain,
            coupling: OnePoleHighPass::new(cutoff_hz, sample_rate_hz),
            mixer_ohms,
            grid_leak_ohms,
        }
    }

    #[must_use]
    pub fn node_gain(&self) -> f64 {
        self.node_gain
    }

    #[must_use]
    pub fn mixer_ohms(&self) -> f64 {
        self.mixer_ohms
    }

    #[must_use]
    pub fn grid_leak_ohms(&self) -> f64 {
        self.grid_leak_ohms
    }

    pub fn reset(&mut self) {
        self.coupling.reset();
    }

    /// Mezcla las tensiones de salida de tono de ambos canales.
    pub fn process(&mut self, channel_1_volts: f64, channel_2_volts: f64) -> f64 {
        self.coupling
            .process(self.node_gain * (channel_1_volts + channel_2_volts))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_gain_matches_two_way_divider() {
        let mixer = ChannelMixer::new(270_000.0, 5e-8, 48_000.0);
        // Con ambos canales iguales: vN = 0.881·v (divisor 270k/270k con 1M).
        let expected = (1.0 / 270_000.0) / (2.0 / 270_000.0 + 1.0 / 1_000_000.0);
        assert!((mixer.node_gain() - expected).abs() < 1e-12);
        assert!((2.0 * expected - 0.881).abs() < 0.01);
    }

    #[test]
    fn one_channel_idle_halves_contribution() {
        let sample_rate = 48_000.0;
        let sine = |index: usize| {
            (2.0 * std::f64::consts::PI * 1_000.0 * index as f64 / sample_rate).sin()
        };

        let mut mixer = ChannelMixer::new(270_000.0, 5e-8, sample_rate);
        let mut both_peak = 0.0_f64;
        for index in 0..96_000 {
            let output = mixer.process(sine(index), sine(index));
            if index > 48_000 {
                both_peak = both_peak.max(output.abs());
            }
        }

        mixer.reset();
        let mut single_peak = 0.0_f64;
        for index in 0..96_000 {
            let output = mixer.process(sine(index), 0.0);
            if index > 48_000 {
                single_peak = single_peak.max(output.abs());
            }
        }

        assert!(
            (both_peak - 2.0 * single_peak).abs() / both_peak < 1e-6,
            "both {both_peak} single {single_peak}"
        );
    }
}
