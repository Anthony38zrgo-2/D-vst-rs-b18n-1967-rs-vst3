//! Etapa de potencia push-pull con 2 × 7027A (Hito C, parte 2).
//!
//! Modelo documentado (véase `docs/MODEL_ASSUMPTIONS.md`):
//! - Pentodos Koren con corriente de placa y pantalla y conducción de rejilla
//!   suave (parámetros ajustados al datasheet RCA 7027-A, B18-010/011).
//! - Bias fija: rejilla de control referida a `−55 V` por `100 kΩ`, con
//!   acoplo desde el inversor por `1 kΩ` de stopper y la impedancia de fuente
//!   del inversor declarada como aproximación.
//! - Alimentación dinámica B18-012 (rieles `set_rails`).
//! - OT-471 por [`OutputTransformerModel`] (B18-013): red diferencial con
//!   DCR primaria y, si están caracterizados, `Lm`/`Lleak`/`Cp` estimados; la
//!   conversión estática del Hito C queda como topología ideal de regresión.
//!
//! ```text
//! vp1 + vp2 = 2·B − Rh·(ip1 + ip2)               (lazo común, CT)
//! vp1 − vp2 = −drop(id),  id = (ip1 − ip2)/2     (red diferencial del OT)
//! ```
//!
//! `Rh` = DCR de media primaria. La continua diferencial queda bloqueada por
//! el transformador físico (`Lm`/`Lleak`) en el modo lineal; el modo ideal
//! conserva el paso alto de 1 Hz declarado.

use crate::numerics::discretization::TptCapacitor;
use crate::stages::output_transformer::{
    OutputTransformerConfig, OutputTransformerError, OutputTransformerModel,
};
use crate::stages::speaker_impedance::ElectricalLoadConfig;
use crate::stages::tube_models::{KorenPentodeModel, PentodePlateCurve};
/// Configuración física de la etapa.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerStageConfig {
    pub supply_v: f64,
    pub screen_supply_v: f64,
    pub bias_v: f64,
    pub transformer: OutputTransformerConfig,
    pub grid_stopper_ohms: f64,
    pub grid_return_ohms: f64,
    /// Impedancia de fuente del inversor hacia la rejilla superior (placa).
    pub grid_source_upper_ohms: f64,
    /// Impedancia de fuente del inversor hacia la rejilla inferior (cátodo).
    pub grid_source_lower_ohms: f64,
    /// Acoplo del inversor a cada rejilla (0.05 µF, con memoria de bloqueo).
    pub coupling_farads: f64,
    pub sample_rate_hz: f64,
}

/// Par de 7027A en push-pull con bias fija.
#[derive(Debug, Clone)]
pub struct PushPullStage {
    config: PowerStageConfig,
    model: KorenPentodeModel,
    /// [Vg1, Vg2, Vp1, Vp2] del último paso resuelto.
    nodes: [f64; 4],
    coupling_upper: TptCapacitor,
    coupling_lower: TptCapacitor,
    transformer: OutputTransformerModel,
    idle_plate_current_a: f64,
    idle_screen_current_a: f64,
    last_plate_current_a: [f64; 2],
    last_screen_current_a: [f64; 2],
    convergence_failures: u64,
}

impl PushPullStage {
    /// Construye la etapa; falla si el modelo del OT está incompleto
    /// (parásitos a medias, véase [`OutputTransformerError`]).
    pub fn new(
        model: KorenPentodeModel,
        config: PowerStageConfig,
    ) -> Result<Self, OutputTransformerError> {
        let dt = 1.0 / config.sample_rate_hz;
        let mut coupling_upper = TptCapacitor::new(config.coupling_farads, dt);
        let mut coupling_lower = TptCapacitor::new(config.coupling_farads, dt);
        // Reposo: lado de fuente a 0 V (AC) y rejilla en el bias; la tensión
        // del capacitor es `0 − bias`.
        coupling_upper.preset_voltage(-config.bias_v);
        coupling_lower.preset_voltage(-config.bias_v);
        let transformer = OutputTransformerModel::new(config.transformer, dt)?;
        Ok(Self {
            config,
            model,
            nodes: [
                config.bias_v,
                config.bias_v,
                config.supply_v,
                config.supply_v,
            ],
            coupling_upper,
            coupling_lower,
            transformer,
            idle_plate_current_a: 0.0,
            idle_screen_current_a: 0.0,
            last_plate_current_a: [0.0; 2],
            last_screen_current_a: [0.0; 2],
            convergence_failures: 0,
        })
    }

    #[must_use]
    pub fn config(&self) -> &PowerStageConfig {
        &self.config
    }

    #[must_use]
    pub fn idle_plate_current_a(&self) -> f64 {
        self.idle_plate_current_a
    }

    #[must_use]
    pub fn idle_screen_current_a(&self) -> f64 {
        self.idle_screen_current_a
    }

    #[must_use]
    pub fn plate_voltage(&self, tube: usize) -> f64 {
        self.nodes[2 + tube.min(1)]
    }

    #[must_use]
    pub fn grid_voltage(&self, tube: usize) -> f64 {
        self.nodes[tube.min(1)]
    }

    #[must_use]
    pub fn plate_current(&self, tube: usize) -> f64 {
        self.last_plate_current_a[tube.min(1)]
    }

    #[must_use]
    pub fn screen_current(&self, tube: usize) -> f64 {
        self.last_screen_current_a[tube.min(1)]
    }

    #[must_use]
    pub fn convergence_failures(&self) -> u64 {
        self.convergence_failures
    }

    /// Actualiza los rieles dinámicos B18-012 (B+, pantalla, bias) sin
    /// reiniciar el estado: el solve de la muestra siguiente los usa.
    pub fn set_rails(&mut self, supply_v: f64, screen_supply_v: f64, bias_v: f64) {
        self.config.supply_v = supply_v;
        self.config.screen_supply_v = screen_supply_v;
        self.config.bias_v = bias_v;
    }

    /// Corriente total de placas (CT, nodo H2) de la última muestra.
    #[must_use]
    pub fn total_plate_current_a(&self) -> f64 {
        self.last_plate_current_a[0] + self.last_plate_current_a[1]
    }

    /// Corriente total de pantallas (nodo H1) de la última muestra.
    #[must_use]
    pub fn total_screen_current_a(&self) -> f64 {
        self.last_screen_current_a[0] + self.last_screen_current_a[1]
    }

    /// Corriente total que sale del nodo bias hacia los retornos de 100 kΩ.
    /// `(Vbias − Vg)/Rg` por tubo; en reposo es ~0 (Vg=Vbias). Negativa =
    /// entra al nodo (rejillas más positivas que el bias, caso con drive).
    #[must_use]
    pub fn total_grid_current_a(&self) -> f64 {
        let bias = self.config.bias_v;
        let r = self.config.grid_return_ohms.max(1.0);
        (bias - self.nodes[0]) / r + (bias - self.nodes[1]) / r
    }

    /// Resuelve el reposo (excitación nula) y registra las corrientes de
    /// reposo del modelo. Devuelve `false` si no converge.
    pub fn solve_operating_point(&mut self) -> bool {
        let _ = self.process(0.0);
        let converged = self.convergence_failures == 0;
        self.idle_plate_current_a =
            0.5 * (self.last_plate_current_a[0] + self.last_plate_current_a[1]);
        self.idle_screen_current_a =
            0.5 * (self.last_screen_current_a[0] + self.last_screen_current_a[1]);
        converged
    }

    /// Procesa una excursión de excitación diferencial simétrica (inversor
    /// ideal) y devuelve la tensión de secundario (carga de 8 Ω).
    pub fn process(&mut self, drive_v: f64) -> f64 {
        self.process_grids(drive_v, -drive_v)
    }

    /// Procesa las dos excitaciones de rejilla por separado (salidas reales
    /// del inversor 7199: placa y cátodo del triodo de carga dividida).
    pub fn process_grids(&mut self, drive_upper_v: f64, drive_lower_v: f64) -> f64 {
        let upper = self.solve_coupled_grid(drive_upper_v, true);
        let lower = self.solve_coupled_grid(drive_lower_v, false);
        self.nodes[0] = upper;
        self.nodes[1] = lower;

        if !self.solve_plates() {
            // Estado congelado en el último paso válido: no se avanza el OT.
            self.convergence_failures += 1;
        }
        self.transformer.secondary_v()
    }

    /// Corriente de magnetización diferencial del OT (diagnóstico B18-013).
    #[must_use]
    pub fn magnetizing_current_a(&self) -> f64 {
        self.transformer.magnetizing_current_a()
    }

    /// Corriente de secundario del último avance (diagnóstico B18-015).
    #[must_use]
    pub fn secondary_current_a(&self) -> f64 {
        self.transformer.secondary_current_a()
    }

    /// Carga activa del secundario (B18-015).
    #[must_use]
    pub fn load_config(&self) -> ElectricalLoadConfig {
        self.transformer.load_config()
    }

    /// Cambia la carga del secundario (B18-015) sin reiniciar el estado del
    /// transformador. Precondición: configuración válida (validada al
    /// construir la etapa).
    pub fn set_load(&mut self, load: ElectricalLoadConfig) {
        self.transformer.set_load(load);
    }

    /// Nodo de rejilla con el acoplo de 0.05 µF como **capacitor TPT**.
    ///
    /// La rama serie es `drive → Rs (fuente + stopper) → C → vg`, y la rejilla
    /// retorna al bias por `Rg` con conducción suave `Ig(vg)`. El equivalente
    /// Norton del capacitor da
    /// `i = (Geq·(drive − vg) − Ieq)/(1 + Geq·Rs)` y la KCL de rejilla
    /// `i = (vg − bias)/Rg + Ig(vg)`. La memoria del capacitor reproduce el
    /// *blocking distortion* cuando la rejilla conduce.
    fn solve_coupled_grid(&mut self, drive_v: f64, upper: bool) -> f64 {
        let bias = self.config.bias_v;
        let source_ohms = if upper {
            self.config.grid_source_upper_ohms
        } else {
            self.config.grid_source_lower_ohms
        };
        let source_resistance = source_ohms + self.config.grid_stopper_ohms;
        let return_conductance = 1.0 / self.config.grid_return_ohms;
        let cap = if upper {
            &self.coupling_upper
        } else {
            &self.coupling_lower
        };
        let conductance = cap.conductance();
        let companion = cap.companion_current();
        let denominator = 1.0 + conductance * source_resistance;

        let current_for =
            |grid_v: f64| -> f64 { (conductance * (drive_v - grid_v) - companion) / denominator };
        let evaluate = |grid_v: f64| -> (f64, f64) {
            let (grid_current, slope) = self.model.grid_current_with_slope(grid_v);
            let value = current_for(grid_v) - (grid_v - bias) * return_conductance - grid_current;
            let derivative = -conductance / denominator - return_conductance - slope;
            (value, derivative)
        };

        let mut grid_v = self.nodes[if upper { 0 } else { 1 }];
        for _ in 0..30 {
            let (value, derivative) = evaluate(grid_v);
            if derivative.abs() < 1e-18 {
                break;
            }
            let step = (-value / derivative).clamp(-40.0, 40.0);
            grid_v += step;
            if step.abs() < 1e-9 {
                break;
            }
        }
        let value_only = |grid_v: f64| {
            current_for(grid_v)
                - (grid_v - bias) * return_conductance
                - self.model.grid_current(grid_v)
        };
        let residual = value_only(grid_v);
        if residual.abs() > 1e-7 {
            // `f` decrece con `vg`: bisección robusta en un bracket amplio.
            let (mut low, mut high) = (bias - 400.0, bias + 200.0);
            for _ in 0..80 {
                let middle = 0.5 * (low + high);
                // Further iterations cannot change this bracket in f64.
                if middle == low || middle == high {
                    break;
                }
                let value = value_only(middle);
                if value > 0.0 {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            grid_v = 0.5 * (low + high);
        }

        let current = current_for(grid_v);
        let source_side = drive_v - current * source_resistance;
        let cap = if upper {
            &mut self.coupling_upper
        } else {
            &mut self.coupling_lower
        };
        cap.update(source_side - grid_v);
        grid_v
    }

    /// Resuelve el par de placas: lazo común del CT (suma) y red diferencial
    /// del OT (B18-013) en el mismo Newton.
    ///
    /// Rango de placas `[−B, 2·B]`: la excursión positiva llega al doble del
    /// riel por el *flyback* inductivo y la negativa puede cruzar el cero en
    /// transitorios (el modelo de tubo ya devuelve corriente nula para
    /// `Vp ≤ 0`); la cota evita pasos de Newton no acotados sin recortar la
    /// red de salida.
    fn solve_plates(&mut self) -> bool {
        let supply = self.config.supply_v;
        let half_resistance = 0.5 * self.config.transformer.dc_primary_winding_ohms;
        let screen_v = self.config.screen_supply_v;
        let (grid_1, grid_2) = (self.nodes[0], self.nodes[1]);

        let curve_1 = self.model.prepare_plate_curve(screen_v, grid_1);
        let curve_2 = self.model.prepare_plate_curve(screen_v, grid_2);
        let mut plate_1 = self.nodes[2].clamp(-2.0 * supply, 3.0 * supply);
        let mut plate_2 = self.nodes[3].clamp(-2.0 * supply, 3.0 * supply);

        // Límite de iteraciones: los transitorios del OT pueden exigir un
        // recorrido largo de las placas (>1000 V) con paso acotado; en los
        // cruces de corte la convergencia puede ser lineal. El camino normal
        // sale en 2-6 iteraciones y el residuo admisible es 1 mV (≈ −100 dB
        // relativo a las tensiones del lazo), inaudible.
        for _ in 0..200 {
            let (current_1, slope_1) = curve_1.current_with_slope(plate_1);
            let (current_2, slope_2) = curve_2.current_with_slope(plate_2);
            let differential_current = 0.5 * (current_1 - current_2);
            let (drop, drop_slope) = self.transformer.differential_drop(differential_current);

            let residual_sum =
                plate_1 + plate_2 - 2.0 * supply + half_resistance * (current_1 + current_2);
            let residual_difference = plate_1 - plate_2 + drop;

            if residual_sum.abs() < CONVERGENCE_TOLERANCE_V
                && residual_difference.abs() < CONVERGENCE_TOLERANCE_V
            {
                return self.publish_plate_solution(
                    plate_1, plate_2, &curve_1, &curve_2, screen_v, grid_1, grid_2,
                );
            }

            let jacobian_11 = 1.0 + half_resistance * slope_1;
            let jacobian_12 = 1.0 + half_resistance * slope_2;
            // `d(drop)/d(p1) = drop_slope·(slope_1/2)`.
            let jacobian_21 = 1.0 + 0.5 * drop_slope * slope_1;
            let jacobian_22 = -1.0 - 0.5 * drop_slope * slope_2;
            let determinant = jacobian_11 * jacobian_22 - jacobian_12 * jacobian_21;
            if determinant.abs() < 1e-18 {
                return false;
            }

            let step_1 =
                (-residual_sum * jacobian_22 + residual_difference * jacobian_12) / determinant;
            let step_2 =
                (-jacobian_11 * residual_difference + jacobian_21 * residual_sum) / determinant;

            plate_1 += step_1.clamp(-80.0, 80.0);
            plate_2 += step_2.clamp(-80.0, 80.0);
            plate_1 = plate_1.clamp(-2.0 * supply, 3.0 * supply);
            plate_2 = plate_2.clamp(-2.0 * supply, 3.0 * supply);

            if step_1.abs() < 1e-9 && step_2.abs() < 1e-9 {
                return self.publish_plate_solution(
                    plate_1, plate_2, &curve_1, &curve_2, screen_v, grid_1, grid_2,
                );
            }
        }

        false
    }

    /// Publica la solución convergida de placas: actualiza nodos, corrientes,
    /// pantallas y avanza el transformador.
    #[allow(clippy::too_many_arguments)]
    fn publish_plate_solution(
        &mut self,
        plate_1: f64,
        plate_2: f64,
        curve_1: &PentodePlateCurve,
        curve_2: &PentodePlateCurve,
        screen_v: f64,
        grid_1: f64,
        grid_2: f64,
    ) -> bool {
        self.nodes[2] = plate_1;
        self.nodes[3] = plate_2;
        self.last_plate_current_a = [
            curve_1.current_with_slope(plate_1).0,
            curve_2.current_with_slope(plate_2).0,
        ];
        self.last_screen_current_a = [
            self.model.screen_current(plate_1, screen_v, grid_1),
            self.model.screen_current(plate_2, screen_v, grid_2),
        ];
        let differential_current =
            0.5 * (self.last_plate_current_a[0] - self.last_plate_current_a[1]);
        self.transformer
            .advance(plate_1 - plate_2, differential_current);
        true
    }
}

/// Tolerancia del residuo del solve de placas (V): 1 mV es inaudible frente a
/// las tensiones del lazo (~1000 V, ≈ −120 dB).
const CONVERGENCE_TOLERANCE_V: f64 = 1e-3;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::test_circuit;

    fn transformer_config(circuit: &crate::circuit::CircuitDefinition) -> OutputTransformerConfig {
        OutputTransformerConfig {
            primary_plate_to_plate_ohms_nominal: circuit
                .output_transformer
                .primary_plate_to_plate_ohms_nominal,
            turns_ratio_8ohm: circuit.output_transformer.turns_ratio_primary_to_8ohm,
            turns_ratio_16ohm: circuit.output_transformer.turns_ratio_primary_to_16ohm,
            dc_primary_winding_ohms: circuit.output_transformer.dc_primary_winding_ohms,
            dc_secondary_8ohm_winding_ohms: circuit
                .output_transformer
                .dc_secondary_8ohm_winding_ohms,
            load: crate::stages::speaker_impedance::ElectricalLoadConfig::Resistive {
                ohms: circuit.speaker.nominal_load_ohms,
            },
            magnetizing_inductance_h: circuit.output_transformer.winding_inductance_h,
            leakage_inductance_h: circuit.output_transformer.leakage_inductance_h,
            parasitic_capacitance_f: circuit.output_transformer.parasitic_capacitance_f,
            core_loss_ohms: circuit.output_transformer.core_loss_ohms,
            magnetizing_current_limit_a: circuit.output_transformer.magnetizing_current_limit_a,
        }
    }

    fn stage() -> PushPullStage {
        let circuit = test_circuit();
        let model = KorenPentodeModel::new(
            circuit
                .tube_models
                .koren_7027a
                .expect("modelo 7027A en calibración"),
        );
        PushPullStage::new(
            model,
            PowerStageConfig {
                supply_v: circuit.power_supply.bplus_idle_vdc,
                screen_supply_v: circuit.power_stage.screen_voltage_v_nominal,
                bias_v: circuit.power_stage.control_grid_voltage_v_nominal,
                transformer: transformer_config(&circuit),
                grid_stopper_ohms: circuit.power_stage.grid_stopper_ohms_each,
                grid_return_ohms: circuit.power_stage.grid_return_ohms_each,
                // Simétricas en el fixture para conservar la antisimetría.
                grid_source_upper_ohms: 47_000.0,
                grid_source_lower_ohms: 47_000.0,
                coupling_farads: circuit.power_stage.inverter_to_grid_coupling_farads_each,
                sample_rate_hz: 48_000.0,
            },
        )
        .expect("modelo del OT-471")
    }

    #[test]
    fn operating_point_is_symmetric_and_plausible() {
        let mut stage = stage();
        assert!(stage.solve_operating_point(), "reposo sin converger");
        let idle = stage.idle_plate_current_a();
        // El JSON no documenta corriente de reposo (`null`): se contrasta solo
        // contra el rango de diseño del modelo y la disipación de la 7027A.
        assert!(idle > 0.002 && idle < 0.06, "reposo {idle} A");
        let dissipation = stage.plate_voltage(0) * idle;
        assert!(dissipation < 30.0, "disipación {dissipation} W");
        assert!((stage.plate_voltage(0) - stage.plate_voltage(1)).abs() < 0.5);
        assert!((stage.plate_voltage(0) - 505.0).abs() < 40.0);
        assert_eq!(stage.convergence_failures(), 0);
    }

    #[test]
    fn idle_grids_sit_at_bias() {
        let mut stage = stage();
        stage.solve_operating_point();
        assert!(
            (stage.grid_voltage(0) + 55.0).abs() < 0.5,
            "{}",
            stage.grid_voltage(0)
        );
        assert!((stage.grid_voltage(1) + 55.0).abs() < 0.5);
    }

    #[test]
    fn output_is_antisymmetric() {
        let mut positive = stage();
        positive.solve_operating_point();
        let mut negative = stage();
        negative.solve_operating_point();

        let mut worst = 0.0_f64;
        for index in 0..200 {
            let drive = index as f64 * 0.25;
            let out_positive = positive.process(drive);
            let out_negative = negative.process(-drive);
            let mismatch = (out_positive + out_negative).abs();
            worst = worst.max(mismatch);
        }
        assert!(worst < 1e-6, "asimetría {worst}");
    }

    #[test]
    fn large_drive_reaches_class_b_cutoff() {
        let mut stage = stage();
        stage.solve_operating_point();
        let mut minimum_current = f64::INFINITY;
        let mut maximum_current = 0.0_f64;
        let mut peak_output = 0.0_f64;
        for index in 0..2_000 {
            let drive = 60.0 * (index as f64 * 0.01).sin();
            let output = stage.process(drive);
            assert!(output.is_finite());
            minimum_current =
                minimum_current.min(stage.plate_current(0).min(stage.plate_current(1)));
            maximum_current =
                maximum_current.max(stage.plate_current(0).max(stage.plate_current(1)));
            peak_output = peak_output.max(output.abs());
        }
        assert!(
            minimum_current < 0.001,
            "no llegó al corte: {minimum_current} A"
        );
        assert!(
            maximum_current > 0.05,
            "sin excursión de corriente: {maximum_current} A"
        );
        assert!(peak_output > 1.0, "salida insuficiente: {peak_output} V");
        assert!(
            stage.convergence_failures() < 50,
            "{}",
            stage.convergence_failures()
        );
    }

    #[test]
    fn grid_conduction_limits_positive_drive() {
        let mut stage = stage();
        stage.solve_operating_point();
        let _ = stage.process(150.0);
        assert!(
            stage.grid_voltage(0) < 5.0,
            "rejilla descontrolada: {}",
            stage.grid_voltage(0)
        );
        assert!(stage.grid_voltage(0) > -10.0);
    }

    #[test]
    fn extreme_drive_stays_finite() {
        let mut stage = stage();
        stage.solve_operating_point();
        for index in 0..5_000 {
            let drive = 300.0 * (index as f64 * 0.02).sin();
            let output = stage.process(drive);
            assert!(output.is_finite());
            assert!(output.abs() < 2_000.0);
        }
        assert!(
            stage.convergence_failures() < 100,
            "{}",
            stage.convergence_failures()
        );
        let _ = std::f64::consts::PI;
    }
}
