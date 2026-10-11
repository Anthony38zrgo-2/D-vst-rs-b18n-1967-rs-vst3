//! Estado temporal y punto de reposo del motor.
//!
//! El reposo se resuelve al inicializar (Newton sobre los triodos) y se
//! registra para diagnóstico; nunca se insertan tensiones nominales a mano en
//! los capacitores. Los contadores de fallo se exponen para que el plugin
//! pueda publicarlos como bandera de diagnóstico.

use crate::stages::phase_splitter::PhaseInverterOperatingPoint;

/// Tensión de reposo de una etapa de triodo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageOperatingPoint {
    pub plate_v: f64,
    pub cathode_v: f64,
}

/// Punto de reposo resuelto del canal 1 (los canales comparten topología).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OperatingPoint {
    pub triode_1: StageOperatingPoint,
    pub triode_2: StageOperatingPoint,
    /// Reposo del inversor 7199 (pentodo + triodo de carga dividida).
    pub phase_inverter: PhaseInverterOperatingPoint,
    /// Tensión de placa de reposo de una 7027A (modelo, no medición).
    pub power_plate_v: f64,
    /// Corriente de reposo de placa por tubo (modelo; el JSON la deja `null`).
    pub power_idle_plate_current_a: f64,
    /// Corriente de reposo de pantalla por tubo (modelo).
    pub power_idle_screen_current_a: f64,
}

/// Estado interno, siempre propio de cada instancia y por canal.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EngineState {
    /// El reposo se resolvió al inicializar y quedó congruente.
    pub operating_point_solved: bool,
    /// Tensiones de reposo de las etapas del previo.
    pub operating_point: Option<OperatingPoint>,
    /// Fallos de convergencia acumulados (no se propagan como `NaN`).
    pub convergence_failures: u64,
}

impl EngineState {
    /// Restablece el estado preservando el reposo resuelto.
    pub fn reset(&mut self) {
        self.convergence_failures = 0;
    }
}
