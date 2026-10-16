//! Discretización estable de elementos reactivos (método trapezoidal / TPT).
//!
//! El capacitor se modela con un equivalente de Norton:
//! `i = Geq·v − Ieq`, con `Geq = 2C/T`. Tras resolver los nodos, el estado se
//! actualiza con `Ieq ← Geq·v + i`. El método es A-estable y exacto para
//! redes lineales de primer orden con excitación constante a tramos.

/// Estado de un capacitor discretizado con TPT.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TptCapacitor {
    capacitance_farads: f64,
    conductance: f64,
    companion_current: f64,
    voltage: f64,
    current: f64,
}

impl TptCapacitor {
    /// Crea el capacitor para un paso `dt_seconds` (en segundos).
    #[must_use]
    pub fn new(capacitance_farads: f64, dt_seconds: f64) -> Self {
        let conductance = 2.0 * capacitance_farads / dt_seconds;
        Self {
            capacitance_farads,
            conductance,
            companion_current: 0.0,
            voltage: 0.0,
            current: 0.0,
        }
    }

    /// Ajusta el paso de integración (cambio de sample rate u oversampling).
    pub fn set_time_step(&mut self, dt_seconds: f64) {
        self.conductance = 2.0 * self.capacitance_farads / dt_seconds;
    }

    pub fn reset(&mut self) {
        self.companion_current = 0.0;
        self.voltage = 0.0;
        self.current = 0.0;
    }

    /// Fija el estado de reposo del capacitor (tensión inicial congruente).
    pub fn preset_voltage(&mut self, voltage: f64) {
        self.voltage = voltage;
        self.current = 0.0;
        self.companion_current = self.conductance * voltage;
    }

    #[must_use]
    pub fn capacitance_farads(&self) -> f64 {
        self.capacitance_farads
    }

    #[must_use]
    pub fn conductance(&self) -> f64 {
        self.conductance
    }

    /// Fuente de Norton del paso actual (`i = Geq·v − Ieq`).
    #[must_use]
    pub fn companion_current(&self) -> f64 {
        self.companion_current
    }

    #[must_use]
    pub fn voltage(&self) -> f64 {
        self.voltage
    }

    /// Actualiza el estado con la tensión resuelta del paso actual.
    pub fn update(&mut self, voltage_now: f64) {
        let current_now = self.conductance * voltage_now - self.companion_current;
        self.voltage = voltage_now;
        self.current = current_now;
        self.companion_current = self.conductance * voltage_now + current_now;
    }
}

/// Método de integración del inductor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InductorIntegration {
    /// Trapezoidal (TPT): `G = T/(2L)`, memoria `i + G·v` (fiel en fase,
    /// puede mantener modos cercanos a Nyquist).
    Trapezoidal,
    /// Euler atrás: `G = T/L`, memoria solo de corriente (disipativo; se usa
    /// en el camino de flujo del OT por estabilidad numérica, B18-013).
    BackwardEuler,
}

/// Estado de un inductor discretizado (TPT trapezoidal o Euler atrás).
///
/// Equivalente de Thevenin/Norton: `i = G·v + Ieq`. Se usa para la inductancia
/// de magnetización y de fuga del transformador de salida (B18-013). En
/// régimen constante el estado permanece, lo que reproduce el bloqueo físico
/// de la componente continua (el flujo no cambia sin tensión aplicada).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TptInductor {
    inductance_h: f64,
    integration: InductorIntegration,
    conductance: f64,
    companion_current: f64,
    voltage: f64,
    current: f64,
}

impl TptInductor {
    /// Crea el inductor trapezoidal para un paso `dt_seconds` (en segundos).
    #[must_use]
    pub fn new(inductance_h: f64, dt_seconds: f64) -> Self {
        Self::with_integration(inductance_h, dt_seconds, InductorIntegration::Trapezoidal)
    }

    /// Crea el inductor con el método de integración pedido.
    #[must_use]
    pub fn with_integration(
        inductance_h: f64,
        dt_seconds: f64,
        integration: InductorIntegration,
    ) -> Self {
        let mut inductor = Self {
            inductance_h,
            integration,
            conductance: 0.0,
            companion_current: 0.0,
            voltage: 0.0,
            current: 0.0,
        };
        inductor.set_time_step(dt_seconds);
        inductor
    }

    /// Ajusta el paso de integración (cambio de sample rate u oversampling).
    pub fn set_time_step(&mut self, dt_seconds: f64) {
        let factor = match self.integration {
            InductorIntegration::Trapezoidal => 0.5,
            InductorIntegration::BackwardEuler => 1.0,
        };
        self.conductance = factor * dt_seconds / self.inductance_h;
    }

    pub fn reset(&mut self) {
        self.companion_current = 0.0;
        self.voltage = 0.0;
        self.current = 0.0;
    }

    /// Fija el estado de reposo del inductor (corriente inicial, tensión 0).
    pub fn preset_current(&mut self, current_a: f64) {
        self.current = current_a;
        self.voltage = 0.0;
        self.companion_current = current_a;
    }

    #[must_use]
    pub fn inductance_h(&self) -> f64 {
        self.inductance_h
    }

    #[must_use]
    pub fn conductance(&self) -> f64 {
        self.conductance
    }

    /// Fuente de Norton del paso actual (`i = G·v + Ieq`).
    #[must_use]
    pub fn companion_current(&self) -> f64 {
        self.companion_current
    }

    #[must_use]
    pub fn voltage(&self) -> f64 {
        self.voltage
    }

    #[must_use]
    pub fn current(&self) -> f64 {
        self.current
    }

    /// Actualiza el estado con la tensión resuelta del paso actual.
    pub fn update(&mut self, voltage_now: f64) {
        let current_now = self.conductance * voltage_now + self.companion_current;
        self.voltage = voltage_now;
        self.current = current_now;
        self.companion_current = match self.integration {
            InductorIntegration::Trapezoidal => current_now + self.conductance * voltage_now,
            InductorIntegration::BackwardEuler => current_now,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rl_step_tracks_analytic_response() {
        // Escalón V → R en serie → L a masa: i(t) = (V/R)·(1 − e^{−tR/L}).
        let r = 100.0_f64;
        let l = 0.01_f64;
        let dt = 1e-5_f64;
        let steps = 5_000;
        let mut inductor = TptInductor::new(l, dt);
        let mut voltage = 0.0_f64;
        for _ in 0..steps {
            // KCL en el nodo R–L: (1 − v)/R = G·v + Ieq.
            let geq = inductor.conductance();
            let ieq = inductor.companion_current();
            let v = (1.0 / r - ieq) / (1.0 / r + geq);
            inductor.update(v);
            voltage = v;
        }
        let analytic = (1.0 - (-(steps as f64) * dt * r / l).exp()) / r;
        assert!(
            (inductor.current() - analytic).abs() < 1e-4,
            "i = {}, analítico = {analytic}",
            inductor.current()
        );
        assert!(voltage < 0.01, "v_L residual {voltage}");
    }

    #[test]
    fn inductor_holds_current_without_voltage() {
        let mut inductor = TptInductor::new(1.0, 1e-4);
        inductor.preset_current(0.5);
        for _ in 0..1_000 {
            inductor.update(0.0);
        }
        assert!((inductor.current() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn rc_lowpass_tracks_analytic_step_response() {
        // V en la entrada a través de R = 1k; C = 1 µF a masa.
        // Respuesta al escalón: v(t) = 1 − e^{−t/RC}.
        let r = 1000.0_f64;
        let c = 1e-6_f64;
        let dt = 1e-4_f64;
        let total = 0.01_f64;

        let mut capacitor = TptCapacitor::new(c, dt);
        let mut voltage = 0.0_f64;
        let steps = (total / dt) as usize;
        for _ in 0..steps {
            // Nodo entre R (a fuente de 1 V) y C a masa.
            // (1/R + Geq)·v = 1/R + Ieq
            let geq = capacitor.conductance();
            let ieq = capacitor.companion_current();
            let v = (1.0 / r + ieq) / (1.0 / r + geq);
            capacitor.update(v);
            voltage = v;
        }
        let analytic = 1.0 - (-total / (r * c)).exp();
        // Error de discretización trapezoidal esperado (~2e-6 aquí).
        assert!(
            (voltage - analytic).abs() < 5e-6,
            "v = {voltage}, analítico = {analytic}"
        );
    }

    #[test]
    fn reset_clears_state() {
        let mut capacitor = TptCapacitor::new(1e-6, 1e-4);
        capacitor.update(1.0);
        capacitor.reset();
        assert_eq!(capacitor.voltage(), 0.0);
        assert_eq!(capacitor.companion_current(), 0.0);
    }
}
