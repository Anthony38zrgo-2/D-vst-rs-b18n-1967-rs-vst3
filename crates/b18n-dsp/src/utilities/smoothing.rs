//! Suavizado de parámetros para automatización sin clics (5–30 ms típico).
//!
//! Implementación exponencial de primer orden, sin asignaciones y apta para el
//! callback de audio.

/// Valor suavizado exponencialmente hacia un objetivo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothedValue {
    current: f32,
    target: f32,
    coeff: f32,
}

impl SmoothedValue {
    /// Crea el suavizado con el valor inicial ya en el objetivo.
    ///
    /// `time_ms <= 0` desactiva el suavizado (salto instantáneo).
    #[must_use]
    pub fn new(initial: f32, sample_rate_hz: f32, time_ms: f32) -> Self {
        let mut value = Self {
            current: initial,
            target: initial,
            coeff: 1.0,
        };
        value.set_time(sample_rate_hz, time_ms);
        value
    }

    /// Recalcula el coeficiente tras un cambio de sample rate.
    pub fn set_time(&mut self, sample_rate_hz: f32, time_ms: f32) {
        if time_ms <= 0.0 || sample_rate_hz <= 0.0 {
            self.coeff = 1.0;
        } else {
            let samples = time_ms * 0.001 * sample_rate_hz;
            self.coeff = 1.0 - (-1.0 / samples).exp();
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    #[must_use]
    pub fn target(&self) -> f32 {
        self.target
    }

    #[must_use]
    pub fn current(&self) -> f32 {
        self.current
    }

    /// Avanza un sample y devuelve el valor suavizado.
    pub fn next_value(&mut self) -> f32 {
        self.current += (self.target - self.current) * self.coeff;
        self.current
    }

    /// Salta directamente al objetivo (p. ej. en `reset`).
    pub fn snap(&mut self) {
        self.current = self.target;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converges_towards_target() {
        let mut smoother = SmoothedValue::new(0.0, 48_000.0, 10.0);
        smoother.set_target(1.0);
        for _ in 0..4_800 {
            smoother.next_value();
        }
        assert!(smoother.current() > 0.98, "current {}", smoother.current());
        assert!(smoother.current() <= 1.0);
    }

    #[test]
    fn zero_time_snaps() {
        let mut smoother = SmoothedValue::new(0.0, 48_000.0, 0.0);
        smoother.set_target(1.0);
        assert_eq!(smoother.next_value(), 1.0);
    }

    #[test]
    fn snap_reaches_target() {
        let mut smoother = SmoothedValue::new(0.0, 48_000.0, 10.0);
        smoother.set_target(0.5);
        smoother.snap();
        assert_eq!(smoother.current(), 0.5);
    }
}
