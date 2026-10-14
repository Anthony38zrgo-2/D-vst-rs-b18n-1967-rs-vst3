//! Triodo 6SL7GT en aproximación lineal de pequeña señal.
//!
//! Modelo de fuente controlada con resistencia de salida y acoplo RC:
//! - Ganancia de tensión: `Av = −µ·Ra / (rp + Ra + (µ+1)·Rk)`.
//! - Impedancia de salida: `Ro = Ra·(rp + (µ+1)·Rk) / (rp + Ra + (µ+1)·Rk)`.
//! - Filtro de acoplo: polo en `1/(2π·C·(Ro + Rcarga))` con la pérdida de
//!   inserción `Rcarga/(Ro + Rcarga)` incluida en la ganancia.
//!
//! Los parámetros `µ` y `rp` provienen de la referencia de pequeña señal del
//! JSON; no caracterizan saturación (Hito C).

use std::f64::consts::PI;

use crate::numerics::discretization::TptCapacitor;
use crate::numerics::nonlinear::{
    solve_with_jacobian as newton_solve, NewtonConfig, NewtonFailure, NewtonReport,
};
use crate::stages::tube_models::KorenTriodeModel;
use crate::utilities::filters::OnePoleHighPass;

/// Configuración Newton declarada del solve por muestra del triodo (Ola 8).
///
/// Residuo hasta 1e-9 A (sobre corrientes de ~mA: seis dígitos, muy por
/// debajo de cualquier diferencia audible). El máximo de iteraciones sube de
/// 24 a 64 (Ola 8): con drive extremo (plegado del acoplo, rejilla cortada)
/// Newton entra en fase de arrastre amortiguado que necesita ~26-40
/// iteraciones antes de la fase cuadrática final; 24 se quedaba cortas y
/// congelaba el sample. El camino rápido apenas cambia: convergiendo, el
/// solver sale en 2-6 iteraciones por la condición de paso (<1e-6 V).
const TRIODE_NEWTON: NewtonConfig = NewtonConfig {
    max_iterations: 64,
    tolerance: 1e-6,
    residual_tolerance: 1e-9,
    max_step: 20.0,
    max_backtracks: 4,
};

/// Etapa de ganancia lineal con acoplo.
#[derive(Debug, Clone, PartialEq)]
pub struct TriodeLinearStage {
    voltage_gain: f64,
    output_impedance_ohms: f64,
    coupling: OnePoleHighPass,
}

impl TriodeLinearStage {
    /// Construye la etapa a partir de los valores físicos de la válvula y su
    /// red de placa/cátodo/acoplo.
    #[must_use]
    pub fn new(
        mu: f64,
        rp_ohms: f64,
        plate_resistor_ohms: f64,
        cathode_resistor_ohms: f64,
        coupling_farads: f64,
        load_ohms: f64,
        sample_rate_hz: f64,
    ) -> Self {
        let degeneration = (mu + 1.0) * cathode_resistor_ohms;
        let denominator = rp_ohms + plate_resistor_ohms + degeneration;
        let raw_gain = mu * plate_resistor_ohms / denominator;
        let output_impedance_ohms = plate_resistor_ohms * (rp_ohms + degeneration) / denominator;
        let total_resistance = output_impedance_ohms + load_ohms;
        let insertion_loss = load_ohms / total_resistance;
        let cutoff_hz = 1.0 / (2.0 * PI * coupling_farads * total_resistance);
        Self {
            voltage_gain: -raw_gain * insertion_loss,
            output_impedance_ohms,
            coupling: OnePoleHighPass::new(cutoff_hz, sample_rate_hz),
        }
    }

    #[must_use]
    pub fn voltage_gain(&self) -> f64 {
        self.voltage_gain
    }

    #[must_use]
    pub fn output_impedance_ohms(&self) -> f64 {
        self.output_impedance_ohms
    }

    pub fn reset(&mut self) {
        self.coupling.reset();
    }

    /// Procesa un sample (tensión de entrada → tensión de salida).
    pub fn process(&mut self, input: f64) -> f64 {
        self.coupling.process(input) * self.voltage_gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_stage() -> TriodeLinearStage {
        // 6SL7 del JSON: µ=70, rp=44 kΩ; 470 kΩ / 5.6 kΩ; acoplo 0.022 µF;
        // carga 1 MΩ (potenciómetro de volumen).
        TriodeLinearStage::new(
            70.0,
            44_000.0,
            470_000.0,
            5_600.0,
            2.2e-8,
            1_000_000.0,
            48_000.0,
        )
    }

    #[test]
    fn gain_matches_analytic_formula() {
        let stage = reference_stage();
        let degeneration = 71.0 * 5_600.0;
        let denominator = 44_000.0 + 470_000.0 + degeneration;
        let raw = 70.0 * 470_000.0 / denominator;
        let ro = 470_000.0 * (44_000.0 + degeneration) / denominator;
        let expected = -raw * 1_000_000.0 / (ro + 1_000_000.0);
        assert!((stage.voltage_gain() - expected).abs() < 1e-9);
        assert!(expected < -20.0, "ganancia inesperada: {expected}");
    }

    #[test]
    fn midband_sine_gain_tracks_dc_gain() {
        let mut stage = reference_stage();
        let sample_rate = 48_000.0;
        let mut peak = 0.0_f64;
        for index in 0..96_000 {
            let t = index as f64 / sample_rate;
            let output = stage.process((2.0 * PI * 2_000.0 * t).sin());
            if index > 48_000 {
                peak = peak.max(output.abs());
            }
        }
        let expected = stage.voltage_gain().abs();
        assert!(
            (peak - expected).abs() / expected < 0.02,
            "peak {peak} vs {expected}"
        );
    }

    #[test]
    fn blocks_dc() {
        let mut stage = reference_stage();
        let mut output = 0.0;
        for _ in 0..48_000 {
            output = stage.process(1.0);
        }
        assert!(output.abs() < 1e-2);
    }
}

/// Configuración física de una etapa de triodo no lineal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TriodeStageConfig {
    pub plate_resistor_ohms: f64,
    pub cathode_resistor_ohms: f64,
    pub grid_leak_ohms: f64,
    pub supply_v: f64,
    /// Capacitor de acoplo entre la entrada y la rejilla; `None` = conexión
    /// directa a través de [`Self::series_resistance_ohms`].
    pub coupling_farads: Option<f64>,
    pub series_resistance_ohms: f64,
    pub sample_rate_hz: f64,
}

/// Red de volumen física en la rejilla del segundo 6SL7 (B18-005):
/// placa del primer triodo (`Vsrc` detrás de `Ro`) → capacitor de acoplo →
/// potenciómetro (Rtop–cursor–Rbottom a masa) → rejilla.
///
/// El solve de Newton ve la red como su Thevenin en la rejilla `(Vth, Rth)`
/// y la corriente de rejilla `Ig(Vgk)` cierra la malla; el capacitor TPT
/// conserva su estado entre muestras.
#[derive(Debug, Clone, Copy, PartialEq)]
struct VolumeNetworkState {
    source_resistance_ohms: f64,
    coupling: TptCapacitor,
    pot_ohms: f64,
    top_ohms: f64,
    bottom_ohms: f64,
}

impl VolumeNetworkState {
    /// Thevenin en el cursor (rejilla) para la fuente `v_src` de esta muestra.
    /// Con el capacitor abierto (`active == false`, reposo DC) la rejilla queda
    /// con la carga del potenciómetro sola.
    fn thevenin(&self, v_src: f64, active: bool) -> (f64, f64) {
        let bottom = self.bottom_ohms.max(1.0);
        let source = self.source_resistance_ohms.max(1.0);
        let cap_resistance = if active {
            1.0 / self.coupling.conductance().max(1e-30)
        } else {
            f64::INFINITY
        };
        let companion = if active {
            self.coupling.companion_current()
        } else {
            0.0
        };
        let denominator = bottom + self.top_ohms + source + cap_resistance;
        let vth = if denominator.is_finite() {
            (v_src * bottom - companion * bottom / self.coupling.conductance()) / denominator
        } else {
            0.0
        };
        let rth = 1.0 / (1.0 / bottom + 1.0 / (self.top_ohms + source + cap_resistance));
        (vth, rth)
    }
}

/// Etapa de triodo resuelta por Newton por muestra (Hito C, parte 1).
///
/// Incógnitas: `[Vg, Vk, Vp]`. La corriente de rejilla no se modela todavía;
/// si Newton no converge se conserva el último estado válido y se incrementa
/// el contador de fallos (política documentada, sin `NaN`).
#[derive(Debug, Clone)]
pub struct NonlinearTriodeStage {
    model: KorenTriodeModel,
    config: TriodeStageConfig,
    coupling: Option<TptCapacitor>,
    volume_network: Option<VolumeNetworkState>,
    newton: NewtonConfig,
    nodes: [f64; 3],
    iterations_last: usize,
    convergence_failures: u64,
    last_input_v: f64,
    last_residual_norm: f64,
    last_failure: Option<NewtonFailure>,
}

impl NonlinearTriodeStage {
    #[must_use]
    pub fn new(model: KorenTriodeModel, config: TriodeStageConfig) -> Self {
        let dt = 1.0 / config.sample_rate_hz;
        Self {
            model,
            config,
            coupling: config
                .coupling_farads
                .map(|farads| TptCapacitor::new(farads, dt)),
            volume_network: None,
            newton: TRIODE_NEWTON,
            nodes: [0.0, 0.0, config.supply_v * 0.5],
            iterations_last: 0,
            convergence_failures: 0,
            last_input_v: 0.0,
            last_residual_norm: 0.0,
            last_failure: None,
        }
    }

    /// Última tensión de entrada y residuo del último solve (diagnóstico).
    #[must_use]
    pub fn last_solve_diagnostics(&self) -> (f64, f64) {
        (self.last_input_v, self.last_residual_norm)
    }

    /// Causa del último fallo de convergencia, si lo hubo (diagnóstico).
    #[must_use]
    pub fn last_failure(&self) -> Option<NewtonFailure> {
        self.last_failure
    }

    #[must_use]
    pub fn model(&self) -> &KorenTriodeModel {
        &self.model
    }

    #[must_use]
    pub fn plate_voltage(&self) -> f64 {
        self.nodes[2]
    }

    #[must_use]
    pub fn cathode_voltage(&self) -> f64 {
        self.nodes[1]
    }

    #[must_use]
    pub fn grid_voltage(&self) -> f64 {
        self.nodes[0]
    }

    #[must_use]
    pub fn iterations_last(&self) -> usize {
        self.iterations_last
    }

    #[must_use]
    pub fn convergence_failures(&self) -> u64 {
        self.convergence_failures
    }

    pub fn reset(&mut self) {
        if let Some(coupling) = &mut self.coupling {
            coupling.reset();
        }
        if let Some(network) = self.volume_network.as_mut() {
            network.coupling.reset();
        }
        self.nodes = [0.0, 0.0, self.config.supply_v * 0.5];
        self.iterations_last = 0;
    }

    /// Actualiza la red de entrada (B18-007): resistencia serie del jack y
    /// carga efectiva de rejilla. La serie `0` (jack directo, `instrument`)
    /// usa un piso numérico de 1 Ω para no singularizar la malla.
    pub fn set_input_network(&mut self, series_ohms: f64, grid_leak_ohms: f64) {
        self.config.series_resistance_ohms = series_ohms.max(1.0);
        self.config.grid_leak_ohms = grid_leak_ohms.max(1.0);
    }

    /// Habilita la red de volumen física (B18-005) en la rejilla: fuente
    /// `Ro` → capacitor de acoplo → potenciómetro `pot_ohms` a masa.
    pub fn enable_volume_network(
        &mut self,
        source_resistance_ohms: f64,
        coupling_farads: f64,
        pot_ohms: f64,
    ) {
        let dt = 1.0 / self.config.sample_rate_hz;
        self.volume_network = Some(VolumeNetworkState {
            source_resistance_ohms: source_resistance_ohms.max(1.0),
            coupling: TptCapacitor::new(coupling_farads, dt),
            pot_ohms,
            top_ohms: 0.5 * pot_ohms,
            bottom_ohms: 0.5 * pot_ohms,
        });
    }

    /// Posición del potenciómetro de volumen como **fracción de resistencia**
    /// del cursor (`volume_fraction`): `0` = cursor a masa, `1` = cursor arriba.
    pub fn set_volume_fraction(&mut self, fraction: f64) {
        if let Some(network) = self.volume_network.as_mut() {
            let fraction = fraction.clamp(0.0, 1.0);
            network.bottom_ohms = fraction * network.pot_ohms;
            network.top_ohms = (1.0 - fraction) * network.pot_ohms;
        }
    }

    /// Resuelve el punto de reposo (entrada nula, capacitores abiertos) y lo
    /// guarda como estado inicial. Debe llamarse al inicializar.
    pub fn solve_operating_point(&mut self) -> NewtonReport {
        let config = self.newton;
        let mut x = [0.0, 1.0, self.config.supply_v * 0.5];
        let report = {
            let evaluate = |state: &[f64], out: &mut [f64], jacobian: &mut [[f64; 3]; 3]| {
                self.residual_and_jacobian(0.0, false, state, out, jacobian);
            };
            newton_solve(&mut x, evaluate, &config)
        };
        if report.converged && x.iter().all(|value| value.is_finite()) {
            self.nodes = x;
            if let Some(coupling) = &mut self.coupling {
                coupling.reset();
            }
            if let Some(network) = self.volume_network.as_mut() {
                network.coupling.reset();
            }
        } else {
            self.convergence_failures += 1;
        }
        report
    }

    /// Procesa un sample (tensión de entrada) y devuelve la tensión de placa.
    pub fn process(&mut self, input_v: f64) -> f64 {
        let config = self.newton;
        let mut x = self.nodes;
        if let Some(network) = &self.volume_network {
            // Arranque en la solución lineal de la red (sin conducción de
            // rejilla). La tolerancia de residuo del Newton está en amperios y
            // la red de volumen tiene corrientes de señal muy pequeñas; sin
            // este arranque, el criterio de convergencia congela la rejilla en
            // pequeña señal (el resto del sistema ya está resuelto).
            x[0] = network.thevenin(input_v, true).0;
        }
        let report = {
            let evaluate = |state: &[f64], out: &mut [f64], jacobian: &mut [[f64; 3]; 3]| {
                self.residual_and_jacobian(input_v, true, state, out, jacobian);
            };
            newton_solve(&mut x, evaluate, &config)
        };

        self.iterations_last = report.iterations;
        self.last_input_v = input_v;
        self.last_residual_norm = report.residual_norm;
        self.last_failure = report.failure;
        if report.converged && x.iter().all(|value| value.is_finite()) {
            self.nodes = x;
            if let Some(network) = self.volume_network.as_ref() {
                // Refina la rejilla con la Ig ya convergida: el criterio de
                // parada del Newton (1e-9 A) deja un error de hasta `1e-9·Rth`
                // (~0,1 mV) que, con el capacitor en serie, se integraría en su
                // estado y aparecería como deriva de continua.
                let (vth, rth) = network.thevenin(input_v, true);
                let grid_current = self
                    .model
                    .grid_current_with_slope(self.nodes[0] - self.nodes[1])
                    .0;
                self.nodes[0] = vth - rth * grid_current;
            }
            if let Some(network) = self.volume_network.as_mut() {
                // Estado del capacitor de acoplo a partir de las tensiones de
                // los nodos internos de la red (S: placa tras `Ro`; T: punta
                // del potenciómetro), con la corriente de rejilla resuelta.
                let grid_v = self.nodes[0];
                let grid_current = self.model.grid_current_with_slope(grid_v - self.nodes[1]).0;
                let bottom = network.bottom_ohms.max(1.0);
                let common = grid_current + grid_v / bottom;
                let source_node = input_v - network.source_resistance_ohms.max(1.0) * common;
                let top_node = grid_v + network.top_ohms * common;
                network.coupling.update(source_node - top_node);
            } else if let Some(coupling) = &mut self.coupling {
                coupling.update(input_v - self.nodes[0]);
            }
            self.nodes[2]
        } else {
            self.convergence_failures += 1;
            self.nodes[2]
        }
    }

    fn residual_and_jacobian(
        &self,
        input_v: f64,
        coupling_active: bool,
        x: &[f64],
        out: &mut [f64],
        jacobian: &mut [[f64; 3]; 3],
    ) {
        let (grid_v, cathode_v, plate_v) = (x[0], x[1], x[2]);
        let (plate_current, d_ip_dvp, d_ip_dvgk) = self
            .model
            .plate_current_with_slopes(plate_v, grid_v - cathode_v);
        let (grid_current, d_ig_dvgk) = self.model.grid_current_with_slope(grid_v - cathode_v);

        let (grid_balance, d_grid_balance_dvg) = if let Some(network) = &self.volume_network {
            let (vth, rth) = network.thevenin(input_v, coupling_active);
            ((vth - grid_v) / rth, -1.0 / rth)
        } else if coupling_active {
            match &self.coupling {
                Some(coupling) => {
                    let capacitor_current =
                        coupling.conductance() * (input_v - grid_v) - coupling.companion_current();
                    (
                        capacitor_current - grid_v / self.config.grid_leak_ohms,
                        -(coupling.conductance() + 1.0 / self.config.grid_leak_ohms),
                    )
                }
                None => {
                    // Piso numérico para el jack directo (serie 0 Ω).
                    let series = self.config.series_resistance_ohms.max(1.0);
                    let leak = self.config.grid_leak_ohms.max(1.0);
                    (
                        (input_v - grid_v) / series - grid_v / leak,
                        -(1.0 / series + 1.0 / leak),
                    )
                }
            }
        } else {
            (
                -grid_v / self.config.grid_leak_ohms.max(1.0),
                -1.0 / self.config.grid_leak_ohms.max(1.0),
            )
        };

        out[0] = grid_balance - grid_current;
        out[1] = plate_current + grid_current - cathode_v / self.config.cathode_resistor_ohms;
        out[2] = (plate_v - self.config.supply_v) / self.config.plate_resistor_ohms + plate_current;

        // Jacobiano 3×3. Filas: [Vg, Vk, Vp].
        // `d_ig_dvgk` es respecto de Vgk = Vg − Vk.
        let grid_row = d_grid_balance_dvg - d_ig_dvgk;
        let cath_slope = -d_ip_dvgk - d_ig_dvgk - 1.0 / self.config.cathode_resistor_ohms;
        let plate_slope = 1.0 / self.config.plate_resistor_ohms + d_ip_dvp;
        *jacobian = [
            [grid_row, d_ig_dvgk, 0.0],
            [d_ip_dvgk + d_ig_dvgk, cath_slope, d_ip_dvp],
            [d_ip_dvgk, -d_ip_dvgk, plate_slope],
        ];
    }
}

/// Impedancia de salida estimada de una etapa de cátodo común (para cargar la
/// red de tono): `Ro = Ra·(rp + (µ+1)·Rk) / (rp + Ra + (µ+1)·Rk)`.
#[must_use]
pub fn triode_output_impedance_ohms(
    mu: f64,
    rp_ohms: f64,
    plate_resistor_ohms: f64,
    cathode_resistor_ohms: f64,
) -> f64 {
    let degeneration = (mu + 1.0) * cathode_resistor_ohms;
    let denominator = rp_ohms + plate_resistor_ohms + degeneration;
    plate_resistor_ohms * (rp_ohms + degeneration) / denominator
}

#[cfg(test)]
mod nonlinear_tests {
    use super::*;
    use crate::circuit::test_circuit;

    fn model() -> KorenTriodeModel {
        KorenTriodeModel::new(
            test_circuit()
                .tube_models
                .koren_6sl7
                .expect("modelo en la calibración"),
        )
    }

    fn triode_1_config(coupling_farads: Option<f64>) -> TriodeStageConfig {
        TriodeStageConfig {
            plate_resistor_ohms: 470_000.0,
            cathode_resistor_ohms: 5_600.0,
            grid_leak_ohms: 5_600_000.0,
            supply_v: 380.0,
            coupling_farads,
            series_resistance_ohms: 120_000.0,
            sample_rate_hz: 48_000.0,
        }
    }

    fn triode_2_config() -> TriodeStageConfig {
        TriodeStageConfig {
            plate_resistor_ohms: 220_000.0,
            cathode_resistor_ohms: 2_200.0,
            grid_leak_ohms: 1_000_000.0,
            supply_v: 380.0,
            coupling_farads: None,
            series_resistance_ohms: 250_000.0,
            sample_rate_hz: 48_000.0,
        }
    }

    #[test]
    fn operating_point_matches_documented_triode_1() {
        let mut stage = NonlinearTriodeStage::new(model(), triode_1_config(None));
        let report = stage.solve_operating_point();
        assert!(report.converged, "sin convergencia: {report:?}");
        // JSON: placa 175 V, cátodo 2.4 V (tolerancia por modelo estimado).
        assert!(
            (stage.plate_voltage() - 175.0).abs() / 175.0 < 0.15,
            "placa {} V",
            stage.plate_voltage()
        );
        assert!(
            (stage.cathode_voltage() - 2.4).abs() / 2.4 < 0.30,
            "cátodo {} V",
            stage.cathode_voltage()
        );
        assert_eq!(stage.convergence_failures(), 0);
    }

    #[test]
    fn operating_point_matches_documented_triode_2() {
        let mut stage = NonlinearTriodeStage::new(model(), triode_2_config());
        let report = stage.solve_operating_point();
        assert!(report.converged, "sin convergencia: {report:?}");
        // JSON: placa 185 V, cátodo 2.0 V.
        assert!(
            (stage.plate_voltage() - 185.0).abs() / 185.0 < 0.20,
            "placa {} V",
            stage.plate_voltage()
        );
        assert!(
            (stage.cathode_voltage() - 2.0).abs() / 2.0 < 0.35,
            "cátodo {} V",
            stage.cathode_voltage()
        );
    }

    #[test]
    fn strong_signal_is_finite_with_bounded_failures() {
        let mut stage = NonlinearTriodeStage::new(model(), triode_1_config(Some(2.2e-8)));
        stage.solve_operating_point();
        let mut peak = 0.0_f64;
        for index in 0..48_000 {
            let t = index as f64 / 48_000.0;
            let output = stage.process(10.0 * (2.0 * PI * 100.0 * t).sin());
            assert!(output.is_finite(), "salida no finita en {index}");
            peak = peak.max(output.abs());
        }
        assert!(peak < 1_000.0, "salida descontrolada: {peak}");
        assert!(
            stage.convergence_failures() < 100,
            "fallos de convergencia: {}",
            stage.convergence_failures()
        );
    }

    #[test]
    fn coupling_capacitor_blocks_dc() {
        let config = triode_1_config(Some(2.2e-8));
        let mut stage = NonlinearTriodeStage::new(model(), config);
        stage.solve_operating_point();
        let reference = stage.plate_voltage();
        let mut output = 0.0;
        for _ in 0..96_000 {
            output = stage.process(1.0); // 1 V de continua en la entrada
        }
        assert!(
            (output - reference).abs() < 2.0,
            "la continua no se bloqueó: {output} vs {reference}"
        );
    }

    #[test]
    fn debug_high_drive() {
        let mut stage = NonlinearTriodeStage::new(model(), triode_1_config(Some(2.2e-8)));
        stage.solve_operating_point();
        for index in 0..4000 {
            let t = index as f64 / 48_000.0;
            let input = 6.0 * (2.0 * PI * 7_300.0 * t).sin();
            let output = stage.process(input);
            if stage.convergence_failures() > 0 {
                println!(
                    "fallo en {index}: input {input:.3}, Vg {:.3}, Vk {:.3}, Vp {:.3}, \
                     salida {output:.3}, iteraciones {}",
                    stage.grid_voltage(),
                    stage.cathode_voltage(),
                    stage.plate_voltage(),
                    stage.iterations_last()
                );
                break;
            }
        }
        assert!(stage.convergence_failures() == 0, "falló el debug");
    }

    #[test]
    fn output_impedance_matches_linear_formula() {
        let impedance = triode_output_impedance_ohms(70.0, 44_000.0, 470_000.0, 5_600.0);
        assert!(
            (impedance - 227_700.0).abs() / 227_700.0 < 0.01,
            "{impedance}"
        );
    }
}
