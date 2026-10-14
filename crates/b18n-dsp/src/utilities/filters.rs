//! Filtros de primer orden para acoplos RC (discretización bilineal).
//!
//! El polo se calcula a partir de la frecuencia de corte física
//! `fc = 1/(2π·R·C)`; el ancho de banda y la fase reproducen el
//! comportamiento del acoplo lineal.

/// Paso alto de primer orden (acoplo entre etapas).
///
/// Discretización bilineal de `H(s) = s/(s + ωc)`:
/// `y[n] = b·(x[n] − x[n−1]) + a·y[n−1]`, con `b = 2/(2+ωc·T)` y
/// `a = (2−ωc·T)/(2+ωc·T)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OnePoleHighPass {
    feed_forward: f64,
    feedback: f64,
    x_prev: f64,
    y_prev: f64,
}

impl OnePoleHighPass {
    /// Crea el filtro con la frecuencia de corte física en Hz.
    #[must_use]
    pub fn new(cutoff_hz: f64, sample_rate_hz: f64) -> Self {
        let mut filter = Self {
            feed_forward: 0.0,
            feedback: 1.0,
            x_prev: 0.0,
            y_prev: 0.0,
        };
        filter.set_cutoff(cutoff_hz, sample_rate_hz);
        filter
    }

    /// Recalcula los coeficientes (cambio de sample rate/oversampling).
    pub fn set_cutoff(&mut self, cutoff_hz: f64, sample_rate_hz: f64) {
        let dt = 1.0 / sample_rate_hz;
        let omega_t = 2.0 * std::f64::consts::PI * cutoff_hz * dt;
        self.feed_forward = 2.0 / (2.0 + omega_t);
        self.feedback = (2.0 - omega_t) / (2.0 + omega_t);
    }

    pub fn reset(&mut self) {
        self.x_prev = 0.0;
        self.y_prev = 0.0;
    }

    /// Estado de reposo para una entrada constante: salida 0.
    pub fn preset(&mut self, input: f64) {
        self.x_prev = input;
        self.y_prev = 0.0;
    }

    /// Procesa un sample.
    pub fn process(&mut self, input: f64) -> f64 {
        let output = self.feed_forward * (input - self.x_prev) + self.feedback * self.y_prev;
        self.x_prev = input;
        self.y_prev = output;
        output
    }
}

/// Paso bajo de primer orden (TPT/bilineal con prewarp).
///
/// Discretización bilineal con prewarp de la frecuencia de corte
/// `H(s) = 1/(1 + s/ωc)`:
/// `G = g/(1+g)`, `v = (x − s)·G`, `y = v + s`, `s = y + v` con
/// `g = tan(π·fc/fs)`. La magnitud vale **exactamente −3 dB en `fc`** para
/// cualquier tasa de muestreo (a diferencia del polo apareado por Euler
/// implícito, cuya fase en alta frecuencia se alejaba del RC analógico).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OnePoleLowPass {
    gain: f64,
    state: f64,
}

impl OnePoleLowPass {
    #[must_use]
    pub fn new(cutoff_hz: f64, sample_rate_hz: f64) -> Self {
        let mut filter = Self {
            gain: 0.5,
            state: 0.0,
        };
        filter.set_cutoff(cutoff_hz, sample_rate_hz);
        filter
    }

    pub fn set_cutoff(&mut self, cutoff_hz: f64, sample_rate_hz: f64) {
        let g = (std::f64::consts::PI * cutoff_hz / sample_rate_hz).tan();
        self.gain = g / (1.0 + g);
    }

    pub fn reset(&mut self) {
        self.state = 0.0;
    }

    pub fn process(&mut self, input: f64) -> f64 {
        let delta = (input - self.state) * self.gain;
        let output = delta + self.state;
        self.state = output + delta;
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_pass_passes_high_frequencies() {
        let mut filter = OnePoleHighPass::new(100.0, 48_000.0);
        let mut peak = 0.0_f64;
        for index in 0..48_000 {
            let t = index as f64 / 48_000.0;
            let input = (2.0 * std::f64::consts::PI * 4_000.0 * t).sin();
            let output = filter.process(input);
            if index > 10_000 {
                peak = peak.max(output.abs());
            }
        }
        assert!(peak > 0.9, "el paso alto atenuó la banda de paso: {peak}");
    }

    #[test]
    fn high_pass_blocks_dc_at_steady_state() {
        let mut filter = OnePoleHighPass::new(100.0, 48_000.0);
        let mut output = 0.0;
        for _ in 0..48_000 {
            output = filter.process(1.0);
        }
        assert!(output.abs() < 1e-3, "DC residual: {output}");
    }

    #[test]
    fn low_pass_tracks_dc() {
        let mut filter = OnePoleLowPass::new(1_000.0, 48_000.0);
        let mut output = 0.0;
        for _ in 0..48_000 {
            output = filter.process(1.0);
        }
        assert!((output - 1.0).abs() < 1e-6);
    }

    #[test]
    fn low_pass_is_minus_three_db_at_cutoff_for_every_rate() {
        for rate_hz in [44_100.0, 48_000.0, 96_000.0, 192_000.0] {
            let cutoff_hz = 2_500.0;
            let mut filter = OnePoleLowPass::new(cutoff_hz, rate_hz);
            let frames = 96_000;
            let omega = 2.0 * std::f64::consts::PI * cutoff_hz / rate_hz;
            let tail = frames / 2;
            let (mut re, mut im) = (0.0, 0.0);
            for index in 0..frames {
                let input = (omega * index as f64).sin();
                let output = filter.process(input);
                if index >= tail {
                    let angle = omega * index as f64;
                    re += output * angle.cos();
                    im -= output * angle.sin();
                }
            }
            let magnitude = re.hypot(im) / (tail as f64 / 2.0);
            let db = 20.0 * magnitude.log10();
            assert!(
                (db + 3.0103).abs() < 0.05,
                "tasa {rate_hz}: {db:.4} dB en fc"
            );
        }
    }
}
