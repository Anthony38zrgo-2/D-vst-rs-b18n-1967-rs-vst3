//! Etapa de potencia push-pull con 2 × 7027A (Hito C, parte 2).
//!
//! Modelo documentado (véase `docs/MODEL_ASSUMPTIONS.md`):
//! - Pentodos Koren con corriente de placa y pantalla y conducción de rejilla
//!   suave (el modelo es el de un tubo padre: 6L6GC para la 7027A).
//! - Bias fija: rejilla de control referida a `−55 V` por `100 kΩ`, con
//!   acoplo desde el inversor por `1 kΩ` de stopper y la impedancia de fuente
//!   del inversor declarada como aproximación.
//! - Alimentación **rígida** (B+ 505 V en la toma central y pantallas a 500 V);
//!   la dinámica de B+/bias/pantalla llega en el Hito D.
//! - OT ideal con toma central, DCR de primario repartida, DCR de secundario y
//!   carga nominal:
//!
//! ```text
//! vp1 + vp2 = 2·B − Rh·(ip1 + ip2)
//! vp1 − vp2 = −(Rh + Raa/2)·(ip1 − ip2)
//! vsec      = (vp1 − vp2)/N · Rc/(Rc + Rdc_s)
//! ```
//!
//! `Rh` = DCR de media primaria, `Raa` = impedancia placa-placa, `N` = relación
//! de transformación. La derivación está en la documentación del Hito C.

use crate::numerics::discretization::TptCapacitor;
use crate::stages::tube_models::KorenPentodeModel;
use crate::utilities::filters::OnePoleHighPass;

/// Corte del bloqueo de continua del transformador de salida (asumido; la
/// inductancia real de primario no está documentada y llega con P2). A 1 Hz
/// no afecta a la banda de audio y evita que el lazo de NFB (o la salida)
/// arrastren componente continua.
const OT_DC_BLOCK_CUTOFF_HZ: f64 = 1.0;

/// Configuración física de la etapa.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerStageConfig {
    pub supply_v: f64,
    pub screen_supply_v: f64,
    pub bias_v: f64,
    pub plate_to_plate_ohms: f64,
    pub dc_primary_half_ohms: f64,
    pub dc_secondary_ohms: f64,
    pub turns_ratio: f64,
    pub load_ohms: f64,
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
    output_coupling: OnePoleHighPass,
    idle_plate_current_a: f64,
    idle_screen_current_a: f64,
    last_plate_current_a: [f64; 2],
    last_screen_current_a: [f64; 2],
    convergence_failures: u64,
}

impl PushPullStage {
    #[must_use]
    pub fn new(model: KorenPentodeModel, config: PowerStageConfig) -> Self {
        let dt = 1.0 / config.sample_rate_hz;
        let mut coupling_upper = TptCapacitor::new(config.coupling_farads, dt);
        let mut coupling_lower = TptCapacitor::new(config.coupling_farads, dt);
        // Reposo: lado de fuente a 0 V (AC) y rejilla en el bias; la tensión
        // del capacitor es `0 − bias`.
        coupling_upper.preset_voltage(-config.bias_v);
        coupling_lower.preset_voltage(-config.bias_v);
        Self {
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
            output_coupling: OnePoleHighPass::new(OT_DC_BLOCK_CUTOFF_HZ, config.sample_rate_hz),
            idle_plate_current_a: 0.0,
            idle_screen_current_a: 0.0,
            last_plate_current_a: [0.0; 2],
            last_screen_current_a: [0.0; 2],
            convergence_failures: 0,
        }
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
            self.convergence_failures += 1;
        }

        let difference = self.nodes[2] - self.nodes[3];
        let secondary = difference / self.config.turns_ratio * self.config.load_ohms
            / (self.config.load_ohms + self.config.dc_secondary_ohms);
        // El transformador no pasa continua: paso alto documentado (1 Hz).
        self.output_coupling.process(secondary)
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

    /// Resuelve el par de placas con las dos ecuaciones del OT ideal con DCR.
    fn solve_plates(&mut self) -> bool {
        let supply = self.config.supply_v;
        let half_resistance = self.config.dc_primary_half_ohms;
        let differential_resistance = half_resistance + 0.5 * self.config.plate_to_plate_ohms;
        let screen_v = self.config.screen_supply_v;
        let (grid_1, grid_2) = (self.nodes[0], self.nodes[1]);

        let curve_1 = self.model.prepare_plate_curve(screen_v, grid_1);
        let curve_2 = self.model.prepare_plate_curve(screen_v, grid_2);
        let mut plate_1 = self.nodes[2].clamp(0.0, 2.0 * supply);
        let mut plate_2 = self.nodes[3].clamp(0.0, 2.0 * supply);

        for _ in 0..40 {
            let (current_1, slope_1) = curve_1.current_with_slope(plate_1);
            let (current_2, slope_2) = curve_2.current_with_slope(plate_2);

            let residual_sum =
                plate_1 + plate_2 - 2.0 * supply + half_resistance * (current_1 + current_2);
            let residual_difference =
                plate_1 - plate_2 + differential_resistance * (current_1 - current_2);

            let jacobian_11 = 1.0 + half_resistance * slope_1;
            let jacobian_12 = 1.0 + half_resistance * slope_2;
            let jacobian_21 = 1.0 + differential_resistance * slope_1;
            let jacobian_22 = -1.0 - differential_resistance * slope_2;
            let determinant = jacobian_11 * jacobian_22 - jacobian_12 * jacobian_21;
            if determinant.abs() < 1e-18 {
                return false;
            }

            let step_1 =
                (-residual_sum * jacobian_22 + residual_difference * jacobian_12) / determinant;
            let step_2 =
                (-jacobian_11 * residual_difference + jacobian_21 * residual_sum) / determinant;

            plate_1 += step_1.clamp(-40.0, 40.0);
            plate_2 += step_2.clamp(-40.0, 40.0);
            plate_1 = plate_1.clamp(0.0, 2.0 * supply);
            plate_2 = plate_2.clamp(0.0, 2.0 * supply);

            if step_1.abs() < 1e-9 && step_2.abs() < 1e-9 {
                self.nodes[2] = plate_1;
                self.nodes[3] = plate_2;
                self.last_plate_current_a = [
                    curve_1.current_with_slope(plate_1).0,
                    curve_2.current_with_slope(plate_2).0,
                ];
                self.last_screen_current_a = [
                    self.model.screen_current(screen_v, grid_1),
                    self.model.screen_current(screen_v, grid_2),
                ];
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::test_circuit;

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
                plate_to_plate_ohms: circuit
                    .output_transformer
                    .primary_plate_to_plate_ohms_nominal,
                dc_primary_half_ohms: circuit.output_transformer.dc_primary_winding_ohms / 2.0,
                dc_secondary_ohms: circuit.output_transformer.dc_secondary_8ohm_winding_ohms,
                turns_ratio: circuit.output_transformer.turns_ratio_primary_to_8ohm,
                load_ohms: circuit.speaker.nominal_load_ohms,
                grid_stopper_ohms: circuit.power_stage.grid_stopper_ohms_each,
                grid_return_ohms: circuit.power_stage.grid_return_ohms_each,
                // Simétricas en el fixture para conservar la antisimetría.
                grid_source_upper_ohms: 47_000.0,
                grid_source_lower_ohms: 47_000.0,
                coupling_farads: circuit.power_stage.inverter_to_grid_coupling_farads_each,
                sample_rate_hz: 48_000.0,
            },
        )
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
