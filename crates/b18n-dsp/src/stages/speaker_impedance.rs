//! Carga eléctrica del altavoz de 18" (B18-015).
//!
//! Este módulo modela la **impedancia vista por el secundario del OT** — no la
//! coloración acústica, que vive en [`crate::stages::speaker`]. Dos niveles,
//! sin inventar `Z(f)`:
//!
//! - [`ElectricalLoadConfig::Resistive`] — resistencia nominal del JSON
//!   histórico (`cabinet.nominal_load_ohms = 8 Ω`). Es el **fallback
//!   declarado** mientras no exista una curva de impedancia medida
//!   (`cabinet.speaker_impedance_curve` sigue `null`).
//! - [`ElectricalLoadConfig::LumpedEquivalent`] — equivalente RLC estimado del
//!   altavoz (parámetros `estimated` del overlay de calibración, con método y
//!   fecha). Red en movilidad eléctrica:
//!
//!   ```text
//!   terminales ── Re ── Le ──┬── Res ── retorno
//!                            ├── Les ── retorno
//!                            └── Ces ── retorno
//!   ```
//!
//!   `Res ∥ Les ∥ Ces` es la rama de movimiento (pico de |Z| en `f_s`); `Re` es
//!   la resistencia de la bobina en reposo y `Le` su inductancia. **No es una
//!   curva medida**: sin datos del Cleveland concreto solo hay equivalentes
//!   declarados (véase `docs/MODEL_ASSUMPTIONS.md`).
//!
//! El transformador de salida ve la carga como un puerto Norton
//! `i = G·v + Ieq` cuyos estados avanzan con la tensión de terminal resuelta
//! **dentro del solve** (no se filtra el WAV de salida). La red se discretiza
//! con **TPT/trapezoidal** (inductores y condensador): la carga tiene camino
//! resistivo (`Re`, `Res`), no es un integrador puro como el flujo del OT-471,
//! y el bilineal conserva la respuesta hasta 20 kHz (Euler atrás desviaba
//! ≈1,8 dB a 20 kHz; medido en `tests/reactive_load.rs`).

use crate::numerics::discretization::{InductorIntegration, TptCapacitor, TptInductor};

/// Parámetros del equivalente RLC estimado de un altavoz eléctrico.
///
/// `estimated` por construcción: proceden del overlay de calibración con
/// método y fecha; nunca se presentan como medidos.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LumpedSpeakerParameters {
    /// Resistencia DC de la bobina (Ω).
    pub voice_coil_resistance_ohms: f64,
    /// Inductancia de la bobina (H); domina la subida de |Z| en agudos.
    pub voice_coil_inductance_h: f64,
    /// Resistencia de la rama de movimiento (Ω): fija el pico en `f_s`.
    pub motional_resistance_ohms: f64,
    /// Inductancia de la rama de movimiento (H).
    pub motional_inductance_h: f64,
    /// Condensador de la rama de movimiento (F).
    pub motional_capacitance_farads: f64,
}

impl LumpedSpeakerParameters {
    /// Frecuencia de resonancia de la rama de movimiento (Hz).
    #[must_use]
    pub fn resonance_hz(&self) -> f64 {
        1.0 / (2.0
            * std::f64::consts::PI
            * (self.motional_inductance_h * self.motional_capacitance_farads).sqrt())
    }

    /// Factor de calidad del equivalente (`Q = Res/(ω_s·Les)`).
    #[must_use]
    pub fn motional_q(&self) -> f64 {
        let omega = 2.0 * std::f64::consts::PI * self.resonance_hz();
        self.motional_resistance_ohms / (omega * self.motional_inductance_h)
    }

    /// Pico de la impedancia en la resonancia (`Re + Res`, Ω).
    #[must_use]
    pub fn peak_impedance_ohms(&self) -> f64 {
        self.voice_coil_resistance_ohms + self.motional_resistance_ohms
    }

    /// Todos los parámetros positivos y finitos (precondición de la red).
    #[must_use]
    pub fn is_physically_valid(&self) -> bool {
        [
            self.voice_coil_resistance_ohms,
            self.voice_coil_inductance_h,
            self.motional_resistance_ohms,
            self.motional_inductance_h,
            self.motional_capacitance_farads,
        ]
        .iter()
        .all(|value| value.is_finite() && *value > 0.0)
    }

    /// Respuesta compleja continua `Z(f)` del equivalente (referencia
    /// analítica para contraste y documentación, `(re, im)` en ohmios).
    #[must_use]
    pub fn impedance_at_hertz(&self, frequency_hz: f64) -> (f64, f64) {
        let omega = 2.0 * std::f64::consts::PI * frequency_hz;
        let (mut re, mut im) = (
            self.voice_coil_resistance_ohms,
            omega * self.voice_coil_inductance_h,
        );
        // Rama de movimiento en paralelo: Y_mot = 1/Res + 1/(jωLes) + jωCes.
        let (y_re, y_im) = (
            1.0 / self.motional_resistance_ohms,
            omega * self.motional_capacitance_farads - 1.0 / (omega * self.motional_inductance_h),
        );
        let norm = y_re * y_re + y_im * y_im;
        re += y_re / norm;
        im -= y_im / norm;
        (re, im)
    }
}

/// Carga eléctrica del secundario del OT.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ElectricalLoadConfig {
    /// Resistencia constante (fallback declarado del JSON histórico).
    Resistive { ohms: f64 },
    /// Equivalente RLC estimado (overlay `speaker.reactive_load.*`).
    LumpedEquivalent(LumpedSpeakerParameters),
}

impl ElectricalLoadConfig {
    /// Resistencia equivalente para el modo ideal/regresión (a continua, el
    /// equivalente RLC se reduce a `Re`).
    #[must_use]
    pub fn nominal_ohms(&self) -> f64 {
        match self {
            Self::Resistive { ohms } => *ohms,
            Self::LumpedEquivalent(parameters) => parameters.voice_coil_resistance_ohms,
        }
    }

    /// Valida los parámetros y devuelve `false` para redes degeneradas.
    #[must_use]
    pub fn is_physically_valid(&self) -> bool {
        match self {
            Self::Resistive { ohms } => ohms.is_finite() && *ohms > 0.0,
            Self::LumpedEquivalent(parameters) => parameters.is_physically_valid(),
        }
    }

    /// Etiqueta de procedencia para diagnósticos y reportes.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Resistive { .. } => "resistive",
            Self::LumpedEquivalent(_) => "lumped-estimated",
        }
    }
}

/// Estado de la red de carga, discretizada como puerto Norton.
#[derive(Debug, Clone, Copy, PartialEq)]
enum LoadNetwork {
    Resistive,
    Lumped {
        coil: TptInductor,
        motional_inductance: TptInductor,
        motional_capacitance: TptCapacitor,
    },
}

/// Carga discretizada: `i = G·v + Ieq` en los terminales del altavoz.
///
/// Los coeficientes se derivan una vez por `dt` y configuración (constantes en
/// el tiempo); solo la fuente de Norton evoluciona con los estados.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElectricalLoad {
    network: LoadNetwork,
    /// `Re` (0 en el modo resistivo).
    voice_coil_resistance_ohms: f64,
    /// Conductancia de terminal `G = g_le/den`.
    conductance: f64,
    /// `1/den` con `den = 1 + g_le·(Re + 1/g_mot)` (1 en el modo resistivo).
    inverse_denominator: f64,
    /// Factor `g_le/(g_mot·den)` del estado de la rama de movimiento.
    motional_gain: f64,
    /// Conductancia del nodo de movimiento `g_res + g_les + g_ces`.
    motional_conductance: f64,
    companion_current: f64,
    voltage: f64,
    current: f64,
}

impl ElectricalLoad {
    /// Crea la red de carga para un paso `dt_seconds`.
    ///
    /// Precondición: `config.is_physically_valid()` (validado en el overlay de
    /// calibración y en `OutputTransformerModel::new`).
    #[must_use]
    pub fn new(config: ElectricalLoadConfig, dt_seconds: f64) -> Self {
        match config {
            ElectricalLoadConfig::Resistive { ohms } => Self {
                network: LoadNetwork::Resistive,
                voice_coil_resistance_ohms: 0.0,
                conductance: 1.0 / ohms.max(1e-9),
                inverse_denominator: 1.0,
                motional_gain: 0.0,
                motional_conductance: 0.0,
                companion_current: 0.0,
                voltage: 0.0,
                current: 0.0,
            },
            ElectricalLoadConfig::LumpedEquivalent(parameters) => {
                // TPT/trapezoidal en la carga (a diferencia del flujo del OT):
                // aquí no hay integrador puro sin amortiguación —`Re`/`Res`
                // cierran el camino resistivo— y el bilineal conserva la fase
                // del altavoz hasta bien entrada la banda audible, donde Euler
                // atrás desviaba ≈1,8 dB a 20 kHz (medido, B18-015).
                let coil = TptInductor::with_integration(
                    parameters.voice_coil_inductance_h,
                    dt_seconds,
                    InductorIntegration::Trapezoidal,
                );
                let motional_inductance = TptInductor::with_integration(
                    parameters.motional_inductance_h,
                    dt_seconds,
                    InductorIntegration::Trapezoidal,
                );
                let motional_capacitance =
                    TptCapacitor::new(parameters.motional_capacitance_farads, dt_seconds);
                let motional_conductance = 1.0 / parameters.motional_resistance_ohms.max(1e-9)
                    + motional_inductance.conductance()
                    + motional_capacitance.conductance();
                let series_ohms =
                    parameters.voice_coil_resistance_ohms + 1.0 / motional_conductance;
                let denominator = 1.0 + coil.conductance() * series_ohms;
                Self {
                    network: LoadNetwork::Lumped {
                        coil,
                        motional_inductance,
                        motional_capacitance,
                    },
                    voice_coil_resistance_ohms: parameters.voice_coil_resistance_ohms,
                    conductance: coil.conductance() / denominator,
                    inverse_denominator: 1.0 / denominator,
                    motional_gain: coil.conductance() / (motional_conductance * denominator),
                    motional_conductance,
                    companion_current: 0.0,
                    voltage: 0.0,
                    current: 0.0,
                }
            }
        }
    }

    /// Conductancia de terminal (constante): `i = G·v + Ieq`.
    #[must_use]
    pub fn conductance(&self) -> f64 {
        self.conductance
    }

    /// Fuente de Norton del paso actual.
    #[must_use]
    pub fn companion_current(&self) -> f64 {
        self.companion_current
    }

    /// Tensión de terminal del último avance.
    #[must_use]
    pub fn voltage(&self) -> f64 {
        self.voltage
    }

    /// Corriente de terminal del último avance (positiva hacia la carga).
    #[must_use]
    pub fn current(&self) -> f64 {
        self.current
    }

    /// Vuelve al reposo (sin corriente ni memoria).
    pub fn reset(&mut self) {
        if let LoadNetwork::Lumped {
            coil,
            motional_inductance,
            motional_capacitance,
        } = &mut self.network
        {
            coil.reset();
            motional_inductance.reset();
            motional_capacitance.reset();
        }
        self.companion_current = 0.0;
        self.voltage = 0.0;
        self.current = 0.0;
    }

    /// Avanza la red con la tensión de terminal resuelta y devuelve la
    /// corriente (positiva hacia la carga).
    pub fn advance(&mut self, terminal_voltage_v: f64) -> f64 {
        let current = self.conductance * terminal_voltage_v + self.companion_current;
        if let LoadNetwork::Lumped {
            coil,
            motional_inductance,
            motional_capacitance,
        } = &mut self.network
        {
            // Nodo de movimiento: `i = g_mot·v_m + Ieq_mot`.
            let motional_companion =
                motional_inductance.companion_current() - motional_capacitance.companion_current();
            let node_motional_v = (current - motional_companion) / self.motional_conductance;
            let node_coil_v = terminal_voltage_v - current * self.voice_coil_resistance_ohms;
            coil.update(node_coil_v - node_motional_v);
            motional_inductance.update(node_motional_v);
            motional_capacitance.update(node_motional_v);
            // Fuente de Norton del paso siguiente (KCL ya resuelta arriba).
            let motional_companion =
                motional_inductance.companion_current() - motional_capacitance.companion_current();
            self.companion_current = coil.companion_current() * self.inverse_denominator
                + self.motional_gain * motional_companion;
        }
        self.voltage = terminal_voltage_v;
        self.current = current;
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn estimated_parameters() -> LumpedSpeakerParameters {
        LumpedSpeakerParameters {
            voice_coil_resistance_ohms: 6.5,
            voice_coil_inductance_h: 0.0008,
            motional_resistance_ohms: 23.5,
            motional_inductance_h: 0.0693,
            motional_capacitance_farads: 1.806e-4,
        }
    }

    #[test]
    fn anchors_of_the_estimated_equivalent() {
        let parameters = estimated_parameters();
        assert!(parameters.is_physically_valid());
        assert!((parameters.resonance_hz() - 45.0).abs() < 0.5);
        assert!((parameters.motional_q() - 1.2).abs() < 0.05);
        assert!((parameters.peak_impedance_ohms() - 30.0).abs() < 0.1);
        let (re, im) = parameters.impedance_at_hertz(200.0);
        assert!((re.hypot(im) - 8.15).abs() < 0.2, "|Z|(200) {re}+j{im}");
        let (re, im) = parameters.impedance_at_hertz(1000.0);
        assert!((re.hypot(im) - 7.7).abs() < 0.3, "|Z|(1k) {re}+j{im}");
        assert!(im > 0.0, "por encima de la resonancia domina lo inductivo");
    }

    #[test]
    fn resistive_load_is_a_plain_resistor() {
        let mut load = ElectricalLoad::new(ElectricalLoadConfig::Resistive { ohms: 8.0 }, 1e-5);
        assert!((load.conductance() - 0.125).abs() < 1e-15);
        let current = load.advance(4.0);
        assert!((current - 0.5).abs() < 1e-12);
        assert!((load.current() - 0.5).abs() < 1e-12);
        load.reset();
        assert_eq!(load.current(), 0.0);
        assert_eq!(load.companion_current(), 0.0);
    }

    #[test]
    fn lumped_load_dc_limit_is_the_voice_coil_resistance() {
        // Escalón de tensión: en régimen permanente la red se reduce a Re
        // (Le queda en corto y la rama de movimiento también a continua).
        let parameters = estimated_parameters();
        let mut load = ElectricalLoad::new(
            ElectricalLoadConfig::LumpedEquivalent(parameters),
            1.0 / 192_000.0,
        );
        let mut current = 0.0;
        for _ in 0..2_000_000 {
            current = load.advance(1.0);
        }
        let expected = 1.0 / parameters.voice_coil_resistance_ohms;
        assert!(
            (current - expected).abs() / expected < 1e-3,
            "{current} vs {expected}"
        );
    }

    #[test]
    fn terminal_impedance_matches_the_continuous_reference() {
        // Contraste del puerto Norton discretizado contra la Z(f) analítica
        // (Fourier de un tono puro, descartando el transitorio).
        let parameters = estimated_parameters();
        let dt = 1.0 / 192_000.0;
        for frequency in [45.0, 100.0, 200.0, 1000.0] {
            let mut load =
                ElectricalLoad::new(ElectricalLoadConfig::LumpedEquivalent(parameters), dt);
            let omega = 2.0 * std::f64::consts::PI * frequency;
            let frames = (16.0 / (frequency * dt)) as usize;
            let (mut re, mut im) = (0.0_f64, 0.0_f64);
            for index in 0..frames {
                let angle = omega * index as f64 * dt;
                let voltage = angle.cos();
                let current = load.advance(voltage);
                if index >= frames / 2 {
                    re += current * angle.cos();
                    im -= current * angle.sin();
                }
            }
            let admittance = re.hypot(im) / (frames as f64 / 4.0);
            let (reference_re, reference_im) = parameters.impedance_at_hertz(frequency);
            let reference = 1.0 / reference_re.hypot(reference_im);
            let error = (admittance - reference).abs() / reference;
            assert!(
                error < 0.03,
                "{frequency} Hz: |Y| {admittance:.5} S vs {reference:.5} S ({:.1} %)",
                error * 100.0
            );
        }
    }
}
