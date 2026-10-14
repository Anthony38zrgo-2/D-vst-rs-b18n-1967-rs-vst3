//! Inversor de fase 7199 (P1.4): pentodo de entrada + triodo de carga dividida.
//!
//! Modelo trazado del esquema 5/67 y contrastado con ngspice
//! (`scripts/spice/b18n_7199_meter_loading.cir`, golden en
//! `assets/fixtures/reference/phase_7199/`):
//!
//! - **Pentodo**: placa 270 kΩ y pantalla 470 kΩ desde el carril de previo,
//!   150 kΩ de pantalla a masa, 0.1 µF pantalla-cátodo (TPT), cátodo 1.5 kΩ.
//! - **Acoplo** 0.02 µF del pentodo a la rejilla del triodo, con la malla
//!   10 MΩ placa-rejilla y 3.3 MΩ rejilla-masa.
//! - **Triodo** split-load 47 kΩ/47 kΩ desde el carril trazado (~460 V).
//! - **Salidas** por 0.05 µF hacia las rejillas de las 7027A.
//!
//! Desde **B18-008** el acoplo pentodo→triodo se resuelve de forma **acoplada
//! por muestra** (Newton 6D): el capacitor de 0.02 µF (TPT), la malla
//! 10 MΩ/3.3 MΩ y la conducción de rejilla del triodo forman parte del mismo
//! sistema que el pentodo, con la placa del triodo realimentando a su rejilla
//! en la misma muestra. La aproximación anterior (paso alto de un polo con
//! `alpha` congelado en el reposo) queda solo como **respaldo** si el solve
//! acoplado no converge. Los solves por muestra no asignan memoria.

use crate::circuit::CircuitDefinition;
use crate::numerics::discretization::TptCapacitor;
use crate::numerics::nonlinear::{self as newton, NewtonConfig};
use crate::stages::tube_models::{KorenPentodeModel, KorenTriodeModel};

/// Newton declarado del pentodo (Ola 8): residuo hasta 1e-9 entre volvieros
/// y amperios (6 dígitos, comparable al ruido de bisección anterior); itera
/// y retrocede completos para no sacrificar robustez con drive fuerte.
const PENTODE_NEWTON: NewtonConfig = NewtonConfig {
    max_iterations: 40,
    tolerance: 1e-6,
    residual_tolerance: 1e-9,
    max_step: 20.0,
    max_backtracks: 10,
};

/// Newton del solve acoplado (B18-008). Más fino que el del pentodo: el
/// capacitor de acoplo integra el error de la rejilla/placa muestra a muestra,
/// así que el residuo baja a 1e-10 (con el refinado de rejilla posterior) y el
/// paso se exige a 1e-8.
const COUPLED_NEWTON: NewtonConfig = NewtonConfig {
    max_iterations: 48,
    tolerance: 1e-8,
    residual_tolerance: 1e-10,
    max_step: 20.0,
    max_backtracks: 10,
};

/// Configuración del Newton de reinicio tras la bisección de respaldo: parte
/// del bracket resuelto, solo pule (pantalla/cátodo acoplados).
const PENTODE_RESTART: NewtonConfig = NewtonConfig {
    max_iterations: 16,
    tolerance: 1e-6,
    residual_tolerance: 1e-9,
    max_step: 20.0,
    max_backtracks: 10,
};

/// Configuración física del inversor de fase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseInverter7199Config {
    pub preamp_supply_v: f64,
    pub triode_supply_v: f64,
    pub pentode_plate_ohms: f64,
    pub pentode_screen_feed_ohms: f64,
    pub pentode_screen_ground_ohms: f64,
    pub pentode_cathode_ohms: f64,
    pub screen_bypass_farads: f64,
    pub coupling_farads: f64,
    pub grid_leak_plate_ohms: f64,
    pub grid_leak_ground_ohms: f64,
    pub triode_plate_load_ohms: f64,
    pub triode_cathode_load_ohms: f64,
    /// Resistencia de realimentación desde la toma de 16 Ω al cátodo del
    /// pentodo (22 kΩ). `None` = lazo abierto.
    pub feedback_resistor_ohms: Option<f64>,
    pub sample_rate_hz: f64,
}

/// Tensiones de reposo del inversor (modelo, no medición).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhaseInverterOperatingPoint {
    pub pentode_plate_v: f64,
    pub pentode_screen_v: f64,
    pub pentode_cathode_v: f64,
    pub triode_plate_v: f64,
    pub triode_cathode_v: f64,
    pub triode_grid_v: f64,
}

impl Default for PhaseInverterOperatingPoint {
    fn default() -> Self {
        Self {
            pentode_plate_v: 0.0,
            pentode_screen_v: 0.0,
            pentode_cathode_v: 0.0,
            triode_plate_v: 0.0,
            triode_cathode_v: 0.0,
            triode_grid_v: 0.0,
        }
    }
}

/// Inversor de fase real de la 7199.
#[derive(Debug, Clone)]
pub struct PhaseInverter7199 {
    config: PhaseInverter7199Config,
    pentode: KorenPentodeModel,
    triode: KorenTriodeModel,
    operating_point: PhaseInverterOperatingPoint,
    plate_v: f64,
    screen_v: f64,
    cathode_v: f64,
    screen_cap: TptCapacitor,
    /// Capacitor de acoplo de 0.02 µF pentodo→rejilla del triodo (TPT).
    coupling_cap: TptCapacitor,
    source_upper_ohms: f64,
    source_lower_ohms: f64,
    drive_upper: f64,
    drive_lower: f64,
    /// Estado acoplado `[Vs, Vc, Ip, Vg, It, Vk]` (arranque cálido).
    coupled_state: [f64; 6],
    /// Semilla del estado acoplado en el reposo.
    coupled_state_reset: [f64; 6],
    /// Estado del pentodo `[Vs, Vc, Ip]` para el respaldo.
    pentode_state: [f64; 3],
    /// Semilla del pentodo en el punto de reposo.
    pentode_state_reset: [f64; 3],
    /// Última corriente del triodo split-load (arranque cálido del respaldo).
    triode_current_a: f64,
    /// Iteraciones y convergencia del último solve del pentodo (diagnóstico).
    last_iter_count: usize,
    last_converged: bool,
    /// Causa del último fallo (diagnóstico; `None` si el último solve convergió).
    latest_failure: Option<crate::numerics::nonlinear::NewtonFailure>,
    convergence_failures: u64,
}

impl PhaseInverter7199 {
    /// Construye el inversor desde el `CircuitDefinition` (errata incluida).
    ///
    /// Devuelve `None` si faltan datos trazados (p. ej. la errata no aplicada).
    #[must_use]
    pub fn from_circuit(circuit: &CircuitDefinition, sample_rate_hz: f64) -> Option<Self> {
        let pentode = circuit.tube_models.koren_7199_pentode?;
        let triode = circuit.tube_models.koren_7199_triode?;
        let preamp_supply_v = circuit.tube_models.preamp_supply_v?;
        let triode_supply_v = circuit.phase_inverter.triode_supply_v?;
        let screen_ground = circuit
            .phase_inverter
            .additional_traced_resistors
            .iter()
            .find(|resistor| resistor.to_node.contains("masa"))
            .map(|resistor| resistor.ohms)?;
        let grid_leak_plate = pick_value(&circuit.phase_inverter.large_resistors_ohms, 10.0e6)?;
        let grid_leak_ground = pick_value(&circuit.phase_inverter.large_resistors_ohms, 3.3e6)?;
        let plate_load = *circuit
            .phase_inverter
            .phase_splitter_resistors_ohms
            .first()?;
        let cathode_load = *circuit
            .phase_inverter
            .phase_splitter_resistors_ohms
            .get(1)
            .unwrap_or(&plate_load);

        Some(Self::new(
            KorenPentodeModel::new(pentode),
            KorenTriodeModel::new(triode),
            PhaseInverter7199Config {
                preamp_supply_v,
                triode_supply_v,
                pentode_plate_ohms: circuit.phase_inverter.pentode_plate_resistor_ohms,
                pentode_screen_feed_ohms: circuit.phase_inverter.pentode_screen_feed_resistor_ohms,
                pentode_screen_ground_ohms: screen_ground,
                pentode_cathode_ohms: circuit.phase_inverter.pentode_cathode_resistor_ohms,
                screen_bypass_farads: circuit
                    .phase_inverter
                    .pentode_cathode_bypass_farads
                    .unwrap_or(1e-7),
                coupling_farads: circuit.phase_inverter.pentode_to_triode_coupling_farads,
                grid_leak_plate_ohms: grid_leak_plate,
                grid_leak_ground_ohms: grid_leak_ground,
                triode_plate_load_ohms: plate_load,
                triode_cathode_load_ohms: cathode_load,
                feedback_resistor_ohms: circuit
                    .output_transformer
                    .negative_feedback
                    .present
                    .then_some(
                        circuit
                            .output_transformer
                            .negative_feedback
                            .resistor_from_secondary_ohms_visible,
                    )
                    .filter(|ohms| *ohms > 0.0),
                sample_rate_hz,
            },
        ))
    }

    /// Construye el inversor con modelos y configuración explícitos.
    #[must_use]
    pub fn new(
        pentode: KorenPentodeModel,
        triode: KorenTriodeModel,
        config: PhaseInverter7199Config,
    ) -> Self {
        let dt = 1.0 / config.sample_rate_hz;
        let (operating_point, source_upper, source_lower, _alpha) = solve_operating_point(
            &pentode, &triode, &config,
        )
        .unwrap_or((PhaseInverterOperatingPoint::default(), 0.0, 0.0, 0.0));

        let mut stage = Self {
            config,
            pentode,
            triode,
            operating_point,
            plate_v: operating_point.pentode_plate_v,
            screen_v: operating_point.pentode_screen_v,
            cathode_v: operating_point.pentode_cathode_v,
            screen_cap: TptCapacitor::new(config.screen_bypass_farads, dt),
            coupling_cap: TptCapacitor::new(config.coupling_farads, dt),
            source_upper_ohms: source_upper,
            source_lower_ohms: source_lower,
            drive_upper: 0.0,
            drive_lower: 0.0,
            coupled_state: [0.0; 6],
            coupled_state_reset: [0.0; 6],
            pentode_state: [0.0; 3],
            pentode_state_reset: [0.0; 3],
            triode_current_a: 0.0,
            last_iter_count: 0,
            last_converged: true,
            latest_failure: None,
            convergence_failures: 0,
        };
        stage.reset();
        stage
    }

    #[must_use]
    pub fn config(&self) -> &PhaseInverter7199Config {
        &self.config
    }

    #[must_use]
    pub fn operating_point(&self) -> PhaseInverterOperatingPoint {
        self.operating_point
    }

    #[must_use]
    pub fn operating_point_solved(&self) -> bool {
        self.operating_point.triode_cathode_v > 1.0 && self.operating_point.pentode_plate_v > 1.0
    }

    /// Impedancia de fuente de la salida de placa del triodo (hacia la rejilla).
    #[must_use]
    pub fn output_impedance_upper_ohms(&self) -> f64 {
        self.source_upper_ohms
    }

    /// Impedancia de fuente de la salida de cátodo del triodo.
    #[must_use]
    pub fn output_impedance_lower_ohms(&self) -> f64 {
        self.source_lower_ohms
    }

    /// Tensión de placa del pentodo del último sample (diagnóstico).
    #[must_use]
    pub fn pentode_plate_voltage(&self) -> f64 {
        self.plate_v
    }

    #[must_use]
    pub fn convergence_failures(&self) -> u64 {
        self.convergence_failures
    }

    /// Iteraciones del último solve del pentodo (diagnóstico).
    #[must_use]
    pub fn last_iteration_count(&self) -> usize {
        self.last_iter_count
    }

    /// `true` si el último solve del pentodo convergió (diagnóstico).
    #[must_use]
    pub fn last_converged(&self) -> bool {
        self.last_converged
    }

    /// Causa del último fallo del solve del pentodo (diagnóstico).
    #[must_use]
    pub fn latest_failure(&self) -> Option<crate::numerics::nonlinear::NewtonFailure> {
        self.latest_failure
    }

    /// Restablece al reposo resuelto (capacitores preseteados, arranques
    /// cálidos en el punto de reposo).
    pub fn reset(&mut self) {
        let op = self.operating_point;
        let plate_current =
            (self.config.preamp_supply_v - op.pentode_plate_v) / self.config.pentode_plate_ohms;
        let triode_current =
            (self.config.triode_supply_v - op.triode_plate_v) / self.config.triode_plate_load_ohms;
        self.plate_v = op.pentode_plate_v;
        self.screen_v = op.pentode_screen_v;
        self.cathode_v = op.pentode_cathode_v;
        self.pentode_state = [op.pentode_screen_v, op.pentode_cathode_v, plate_current];
        self.pentode_state_reset = self.pentode_state;
        self.coupled_state = [
            op.pentode_screen_v,
            op.pentode_cathode_v,
            plate_current,
            op.triode_grid_v,
            triode_current,
            op.triode_cathode_v,
        ];
        self.coupled_state_reset = self.coupled_state;
        self.triode_current_a = triode_current;
        self.screen_cap
            .preset_voltage(op.pentode_screen_v - op.pentode_cathode_v);
        // Tensión de reposo del acoplo: placa del pentodo − rejilla del triodo.
        self.coupling_cap
            .preset_voltage(op.pentode_plate_v - op.triode_grid_v);
        self.drive_upper = 0.0;
        self.drive_lower = 0.0;
        self.convergence_failures = 0;
    }

    /// Procesa la tensión AC de la rejilla del pentodo y la tensión de
    /// realimentación (toma de 16 Ω) y devuelve las **desviaciones crudas**
    /// de placa y cátodo del triodo (el acoplo de 0.05 µF y la memoria de
    /// bloqueo viven en la etapa de potencia).
    pub fn process(&mut self, grid_ac_v: f64, feedback_v: f64) -> (f64, f64) {
        let solved = self
            .solve_coupled(grid_ac_v, feedback_v)
            .or_else(|| self.solve_fallback(grid_ac_v, feedback_v));
        let Some((plate, screen, cathode, grid_v, triode_plate, triode_cathode)) = solved else {
            // Conservar el último estado válido (política del motor, sin NaN).
            self.convergence_failures += 1;
            return (self.drive_upper, self.drive_lower);
        };

        self.plate_v = plate;
        self.screen_v = screen;
        self.cathode_v = cathode;
        self.screen_cap.update(screen - cathode);
        self.coupling_cap.update(plate - grid_v);
        self.drive_upper = triode_plate - self.operating_point.triode_plate_v;
        self.drive_lower = triode_cathode - self.operating_point.triode_cathode_v;
        (self.drive_upper, self.drive_lower)
    }

    /// Solve **acoplado** de 6 incógnitas (B18-008):
    /// `x = [Vs, Vc, Ip, Vg, It, Vk]` con
    ///
    /// - pantalla y cátodo del pentodo (como antes, capacitor TPT de 0.1 µF),
    /// - placa del pentodo implícita en `Ip` por su red **real** (270 kΩ y el
    ///   capacitor de acoplo de 0.02 µF hacia la rejilla del triodo),
    /// - rejilla del triodo con la malla 10 MΩ placa-rejilla / 3.3 MΩ a masa y
    ///   la conducción de rejilla `Ig(Vgk)`,
    /// - triodo split-load con placa y cátodo realimentados en la misma
    ///   muestra (`Vp = Bt − 47k·Ip`, `Vk = 47k·(Ip + Ig)`).
    ///
    /// Jacobiano **analítico**; arranque cálido del sample anterior. Devuelve
    /// `None` si no converge (el llamador usa el respaldo declarado).
    fn solve_coupled(
        &mut self,
        grid_ac_v: f64,
        feedback_v: f64,
    ) -> Option<(f64, f64, f64, f64, f64, f64)> {
        let supply = self.config.preamp_supply_v;
        let feed = self.config.pentode_screen_feed_ohms;
        let screen_ground = self.config.pentode_screen_ground_ohms;
        let cathode_resistor = self.config.pentode_cathode_ohms;
        let plate_resistor = self.config.pentode_plate_ohms;
        let screen_g = self.screen_cap.conductance();
        let screen_ieq = self.screen_cap.companion_current();
        let screen_den = 1.0 / feed + 1.0 / screen_ground + screen_g;
        let feedback_g = self
            .config
            .feedback_resistor_ohms
            .map(|ohms| 1.0 / ohms.max(1.0))
            .unwrap_or(0.0);
        let coupling_g = self.coupling_cap.conductance();
        let coupling_ieq = self.coupling_cap.companion_current();
        let plate_series_inv = 1.0 / (1.0 / plate_resistor + coupling_g);
        let grid_plate_r = self.config.grid_leak_plate_ohms.max(1.0);
        let grid_ground_r = self.config.grid_leak_ground_ohms.max(1.0);
        let triode_supply = self.config.triode_supply_v;
        let triode_plate_load = self.config.triode_plate_load_ohms;
        let triode_cathode_load = self.config.triode_cathode_load_ohms;
        // La 10 MΩ placa-rejilla también carga la placa del triodo: la caída en
        // la 47 kΩ la recorre `Ip + (Vp − Vg)/10M` (resuelto implícitamente).
        let plate_ratio = triode_plate_load / grid_plate_r;
        let plate_den = 1.0 + plate_ratio;
        let d_plate_triode_d_current = -triode_plate_load / plate_den;
        let d_plate_triode_d_grid = plate_ratio / plate_den;

        let residual_and_jacobian =
            |state: &[f64], out: &mut [f64], jacobian: &mut [[f64; 6]; 6]| {
                let screen = state[0];
                let cathode = state[1];
                let plate_current = state[2];
                let grid = state[3];
                let triode_current = state[4];
                let triode_cathode = state[5];

                // Placa del pentodo implícita en `Ip` con la carga real del
                // acoplo: `(B/Rp + Gc·Vg + Ieqc − Ip)/(1/Rp + Gc)`.
                let plate = (supply / plate_resistor + coupling_g * grid + coupling_ieq
                    - plate_current)
                    * plate_series_inv;
                let d_plate_d_grid = coupling_g * plate_series_inv;
                let d_plate_d_current = -plate_series_inv;

                let screen_cathode = screen - cathode;
                let grid_cathode = grid_ac_v - cathode;
                let pentode = self.pentode.currents_with_slopes(
                    plate - cathode,
                    screen_cathode,
                    grid_cathode,
                );

                let cap_screen_current = screen_g * screen_cathode - screen_ieq;
                let feedback_current = (feedback_v - cathode) * feedback_g;
                out[0] = screen
                    - (supply / feed + screen_g * cathode + screen_ieq - pentode.screen_a)
                        / screen_den;
                out[1] = cathode_resistor
                    * (plate_current + pentode.screen_a + cap_screen_current + feedback_current)
                    - cathode;
                out[2] = pentode.plate_a - plate_current;

                // Placa del triodo: `(Bt − Ra·It + (Ra/10M)·Vg)/(1 + Ra/10M)`.
                let triode_plate = (triode_supply - triode_plate_load * triode_current
                    + plate_ratio * grid)
                    / plate_den;
                let (triode_plate_a, d_triode_d_pk, d_triode_d_gk) =
                    self.triode.plate_current_with_slopes(
                        triode_plate - triode_cathode,
                        grid - triode_cathode,
                    );
                let (grid_current, d_grid_current_d_gk) =
                    self.triode.grid_current_with_slope(grid - triode_cathode);

                let cap_coupling_current = coupling_g * (plate - grid) - coupling_ieq;
                out[3] = cap_coupling_current + (triode_plate - grid) / grid_plate_r
                    - grid / grid_ground_r
                    - grid_current;
                out[4] = triode_plate_a - triode_current;
                out[5] = triode_cathode - triode_cathode_load * (triode_current + grid_current);

                // Jacobiano 6×6. Columnas: [Vs, Vc, Ip, Vg, It, Vk].
                let d_ig2_d_cathode = -pentode.d_screen_d_screen - pentode.d_screen_d_grid;
                jacobian[0] = [
                    1.0 + pentode.d_screen_d_screen / screen_den,
                    -(screen_g + pentode.d_screen_d_screen + pentode.d_screen_d_grid) / screen_den,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                ];
                jacobian[1] = [
                    cathode_resistor * (pentode.d_screen_d_screen + screen_g),
                    cathode_resistor * (d_ig2_d_cathode - screen_g - feedback_g) - 1.0,
                    cathode_resistor,
                    0.0,
                    0.0,
                    0.0,
                ];
                jacobian[2] = [
                    pentode.d_plate_d_screen,
                    -(pentode.d_plate_d_screen + pentode.d_plate_d_grid + pentode.d_plate_d_plate),
                    pentode.d_plate_d_plate * d_plate_d_current - 1.0,
                    pentode.d_plate_d_plate * d_plate_d_grid,
                    0.0,
                    0.0,
                ];
                jacobian[3] = [
                    0.0,
                    0.0,
                    coupling_g * d_plate_d_current,
                    coupling_g * (d_plate_d_grid - 1.0)
                        + (d_plate_triode_d_grid - 1.0) / grid_plate_r
                        - 1.0 / grid_ground_r
                        - d_grid_current_d_gk,
                    d_plate_triode_d_current / grid_plate_r,
                    d_grid_current_d_gk,
                ];
                jacobian[4] = [
                    0.0,
                    0.0,
                    0.0,
                    d_triode_d_pk * d_plate_triode_d_grid + d_triode_d_gk,
                    d_triode_d_pk * d_plate_triode_d_current - 1.0,
                    -d_triode_d_pk - d_triode_d_gk,
                ];
                jacobian[5] = [
                    0.0,
                    0.0,
                    0.0,
                    -triode_cathode_load * d_grid_current_d_gk,
                    -triode_cathode_load,
                    1.0 + triode_cathode_load * d_grid_current_d_gk,
                ];
            };

        let mut x = self.coupled_state;
        if !x.iter().all(|value| value.is_finite()) {
            x = self.coupled_state_reset;
        }
        let report =
            newton::solve_with_jacobian::<6, _>(&mut x, residual_and_jacobian, &COUPLED_NEWTON);
        self.last_iter_count = report.iterations;
        self.last_converged = report.converged;
        self.latest_failure = report.failure;
        if report.converged && x.iter().all(|value| value.is_finite()) {
            let plate = (supply / plate_resistor + coupling_g * x[3] + coupling_ieq - x[2])
                * plate_series_inv;
            let triode_plate =
                (triode_supply - triode_plate_load * x[4] + plate_ratio * x[3]) / plate_den;
            // Refina la rejilla con la Ig ya convergida: el criterio de parada
            // del Newton (1e-9 A) deja ~1e-9·Rth (≈0,1 µV con Rth≈130 Ω) que
            // el capacitor de acoplo integraría como deriva de continua.
            let grid_current = self.triode.grid_current_with_slope(x[3] - x[5]).0;
            let grid_den = coupling_g + 1.0 / grid_plate_r + 1.0 / grid_ground_r;
            let grid = (coupling_g * plate - coupling_ieq + triode_plate / grid_plate_r
                - grid_current)
                / grid_den;
            x[3] = grid;
            self.coupled_state = x;
            self.pentode_state = [x[0], x[1], x[2]];
            self.triode_current_a = x[4];
            return Some((plate, x[0], x[1], grid, triode_plate, x[5]));
        }
        None
    }

    /// Respaldo declarado (aproximación de un polo de la Ola 8): rejilla del
    /// triodo resuelta **linealmente** desde la red (sin conducción) con la
    /// placa del pentodo y la corriente del triodo del sample anterior, solve
    /// 3D del pentodo con la carga real del acoplo y solve 1D del triodo. Solo
    /// se usa si el solve acoplado no converge.
    fn solve_fallback(
        &mut self,
        grid_ac_v: f64,
        feedback_v: f64,
    ) -> Option<(f64, f64, f64, f64, f64, f64)> {
        let coupling_g = self.coupling_cap.conductance();
        let coupling_ieq = self.coupling_cap.companion_current();
        let r10 = self.config.grid_leak_plate_ohms.max(1.0);
        let r33 = self.config.grid_leak_ground_ohms.max(1.0);
        let triode_current = self.triode_current_a.clamp(
            0.0,
            self.config.triode_supply_v
                / (self.config.triode_plate_load_ohms + self.config.triode_cathode_load_ohms),
        );
        let triode_plate_prev =
            self.config.triode_supply_v - self.config.triode_plate_load_ohms * triode_current;
        let grid_den = coupling_g + 1.0 / r10 + 1.0 / r33;
        let grid = (coupling_g * self.plate_v - coupling_ieq + triode_plate_prev / r10) / grid_den;

        let (plate, screen, cathode, converged) =
            self.solve_pentode_load(grid_ac_v, feedback_v, coupling_g, coupling_ieq, grid);
        if !converged {
            return None;
        }
        let grid = (coupling_g * plate - coupling_ieq + triode_plate_prev / r10) / grid_den;
        let (triode_plate, triode_cathode) = self.solve_triode(grid);
        Some((plate, screen, cathode, grid, triode_plate, triode_cathode))
    }

    /// Solve del pentodo con el capacitor de pantalla TPT y la corriente de
    /// realimentación en el cátodo. Devuelve `(placa, pantalla, cátodo,
    /// convergió)`.
    ///
    /// Sistema acoplado **Newton 3D** (`x = [Vs, Vc, Ip]`, jacobiano
    /// **analítico** de las pendientes de `tube_models`, sin asignaciones) con
    /// **arranque cálido** del sample anterior. La placa es implícita en `Ip`
    /// por la resistencia de placa y la carga del acoplo:
    /// `Vp(Ip) = (B+/Rp + G·Vg + Ieq − Ip)/(1/Rp + G)`.
    ///
    /// Antes era una bisección anidada (~60 iteraciones de cátodo × [punto
    /// fijo de pantalla ~30 + bisección de placa 60] ≈ 5.500 evaluaciones
    /// Koren por muestra); con Newton son ~2-6 en el camino rápido. Si el
    /// Newton estanca (rejilla positiva extrema, la placa sale de rango por
    /// la zona muerta `Vp ≤ 0` del modelo), respaldo: bisección monótona
    /// sobre la corriente dentro del bracket físico de la red de placa
    /// `[0, A]` y Newton de reinicio; si tampoco converge se conserva el
    /// último estado válido (política del motor, sin `NaN`).
    /// `i_fb = (feedback − Vc)/Rf` (realimentación negativa al cátodo,
    /// topología del AN-183 y del raster).
    fn solve_pentode_load(
        &mut self,
        grid_v: f64,
        feedback_v: f64,
        plate_load_g: f64,
        plate_load_ieq: f64,
        plate_load_vg: f64,
    ) -> (f64, f64, f64, bool) {
        let supply = self.config.preamp_supply_v;
        let feed = self.config.pentode_screen_feed_ohms;
        let cathode_resistor = self.config.pentode_cathode_ohms;
        let plate_resistor = self.config.pentode_plate_ohms;
        let conductance = self.screen_cap.conductance();
        let companion = self.screen_cap.companion_current();
        let denominator = 1.0 / feed + 1.0 / self.config.pentode_screen_ground_ohms + conductance;
        let feedback_conductance = self
            .config
            .feedback_resistor_ohms
            .map(|ohms| 1.0 / ohms.max(1.0))
            .unwrap_or(0.0);
        let plate_series = 1.0 / plate_resistor + plate_load_g;
        let plate_for = |current: f64| {
            (supply / plate_resistor + plate_load_g * plate_load_vg + plate_load_ieq - current)
                / plate_series
        };

        let residual_and_jacobian =
            |state: &[f64], out: &mut [f64], jacobian: &mut [[f64; 3]; 3]| {
                let (screen, cathode, current) = (state[0], state[1], state[2]);
                let screen_cathode = screen - cathode;
                let grid_cathode = grid_v - cathode;
                let plate_cathode = plate_for(current) - cathode;
                let currents =
                    self.pentode
                        .currents_with_slopes(plate_cathode, screen_cathode, grid_cathode);
                let cap_current = conductance * screen_cathode - companion;
                let feedback_current = (feedback_v - cathode) * feedback_conductance;

                out[0] = screen
                    - (supply / feed + conductance * cathode + companion - currents.screen_a)
                        / denominator;
                out[1] = cathode_resistor
                    * (current + currents.screen_a + cap_current + feedback_current)
                    - cathode;
                out[2] = currents.plate_a - current;

                // Filas: [pantalla, cátodo, placa] × columnas [Vs, Vc, Ip].
                // Ig2(s−c, g−c): ∂/∂s = F_sk, ∂/∂c = −F_sk − F_gk.
                let d_ig2_d_cathode = -currents.d_screen_d_screen - currents.d_screen_d_grid;
                jacobian[0] = [
                    1.0 + currents.d_screen_d_screen / denominator,
                    -(conductance + currents.d_screen_d_screen + currents.d_screen_d_grid)
                        / denominator,
                    0.0,
                ];
                jacobian[1] = [
                    cathode_resistor * (currents.d_screen_d_screen + conductance),
                    cathode_resistor * (d_ig2_d_cathode - conductance - feedback_conductance) - 1.0,
                    cathode_resistor,
                ];
                // Ip(pk(s), sk, gk) con pk dependiendo de Ip por plate_for.
                jacobian[2] = [
                    currents.d_plate_d_screen,
                    -(currents.d_plate_d_screen
                        + currents.d_plate_d_grid
                        + currents.d_plate_d_plate),
                    -currents.d_plate_d_plate / plate_series - 1.0,
                ];
            };

        let mut x = self.pentode_state;
        if !x.iter().all(|value| value.is_finite()) {
            // Arranque no finito (no debería ocurrir: solo se guarda estado
            // válido): reinicio tibio desde el punto de reposo resuelto.
            x = self.pentode_state_reset;
        }
        let mut report =
            newton::solve_with_jacobian::<3, _>(&mut x, residual_and_jacobian, &PENTODE_NEWTON);
        if !report.converged {
            // Respaldo declarado: la ecuación de placa embebida en el sistema
            // es **estrictamente decreciente** en `Ip` dentro del rango físico
            // `[0, A]` (la red de placa entrega como máximo `A`; a `Ip = A` la
            // placa queda a 0 V y el tubo deja de conducir). Newton estanca
            // cuando el paso cruza la zona `Vp ≤ 0` del modelo con rejilla
            // positiva extrema; la bisección sobre `Ip` con el resto del
            // estado tibio garantiza el cruce y el Newton de reinicio pule la
            // solución completa (pantalla/cátodo acoplados).
            let plate_current_max_a =
                supply / plate_resistor + plate_load_g * plate_load_vg + plate_load_ieq;
            let mut probe_state = x;
            let plate_residual = |state: &[f64; 3]| -> f64 {
                self.pentode.plate_current_cathode(
                    plate_for(state[2]) - state[1],
                    state[0] - state[1],
                    grid_v - state[1],
                ) - state[2]
            };
            let (mut low, mut high) = (0.0_f64, plate_current_max_a);
            probe_state[2] = low;
            if plate_residual(&probe_state) >= 0.0 {
                probe_state[2] = high;
                if plate_residual(&probe_state) <= 0.0 {
                    for _ in 0..48 {
                        let middle = 0.5 * (low + high);
                        probe_state[2] = middle;
                        if plate_residual(&probe_state) >= 0.0 {
                            low = middle;
                        } else {
                            high = middle;
                        }
                        if high - low < 1e-12 {
                            break;
                        }
                    }
                    probe_state[2] = 0.5 * (low + high);
                    let restart = newton::solve_with_jacobian::<3, _>(
                        &mut probe_state,
                        residual_and_jacobian,
                        &PENTODE_RESTART,
                    );
                    if restart.converged && probe_state.iter().all(|value| value.is_finite()) {
                        x = probe_state;
                        report = restart;
                    }
                } else {
                    // Fuera del bracket físico: conserva el último estado
                    // válido (el llamador cuenta el fallo).
                    x = self.pentode_state;
                    report.residual_norm = f64::INFINITY;
                }
            } else {
                // Fuera del bracket físico: conserva el último estado
                // válido (el llamador cuenta el fallo).
                x = self.pentode_state;
                report.residual_norm = f64::INFINITY;
            }
        }
        self.last_iter_count = report.iterations;
        self.last_converged = report.converged;
        self.latest_failure = report.failure;
        if report.converged && x.iter().all(|value| value.is_finite()) {
            self.pentode_state = x;
            let (screen, cathode, current) = (x[0], x[1], x[2]);
            let plate_v = plate_for(current);
            if plate_v.is_finite() && screen.is_finite() && cathode.is_finite() {
                return (plate_v, screen, cathode, true);
            }
        }
        // Conservar el último estado válido (el llamador cuenta el fallo).
        (self.plate_v, self.screen_v, self.cathode_v, false)
    }

    /// Solve del triodo de carga dividida: **Newton 1D** sobre la corriente
    /// con pendiente analítica del modelo (`Ip = f(Vpk, Vgk)` con
    /// `Vp = B − Ra·Ip` y `Vc = Rk·Ip`), arranque cálido y fallback de
    /// bisección monótona con bracket completo y salida temprana.
    fn solve_triode(&mut self, grid_v: f64) -> (f64, f64) {
        let supply = self.config.triode_supply_v;
        let plate_load = self.config.triode_plate_load_ohms;
        let cathode_load = self.config.triode_cathode_load_ohms;
        let high = supply / (plate_load + cathode_load);

        let evaluate = |current: f64| -> (f64, f64) {
            let plate_v = supply - plate_load * current;
            let cathode_v = cathode_load * current;
            let (model, d_ip_dvp, d_ip_dvgk) = self
                .triode
                .plate_current_with_slopes(plate_v - cathode_v, grid_v - cathode_v);
            let value = model - current;
            let derivative =
                d_ip_dvp * -(plate_load + cathode_load) + d_ip_dvgk * -cathode_load - 1.0;
            (value, derivative)
        };

        let mut current = self.triode_current_a.clamp(0.0, high);
        let mut converged = false;
        for _ in 0..30 {
            let (value, derivative) = evaluate(current);
            if derivative.abs() < 1e-18 {
                break;
            }
            let step = (-value / derivative).clamp(-high * 0.5, high * 0.5);
            current = (current + step).clamp(0.0, high);
            if step.abs() < 1e-12 {
                converged = true;
                break;
            }
        }
        let value_only = |current: f64| {
            let plate_v = supply - plate_load * current;
            let cathode_v = cathode_load * current;
            self.triode
                .plate_current(plate_v - cathode_v, grid_v - cathode_v)
                - current
        };
        let residual = value_only(current);
        if !converged || residual.abs() > 1e-12 {
            // Bisección monótona (`g` decrece) con bracket físico completo.
            let (mut low, mut high_b) = (0.0, high);
            for _ in 0..60 {
                let middle = 0.5 * (low + high_b);
                let value = value_only(middle);
                if value > 0.0 {
                    low = middle;
                } else {
                    high_b = middle;
                }
                if high_b - low < 1e-12 {
                    break;
                }
            }
            current = 0.5 * (low + high_b);
        }
        self.triode_current_a = current;
        (supply - plate_load * current, cathode_load * current)
    }
}

fn pick_value(list: &[f64], target: f64) -> Option<f64> {
    list.iter()
        .copied()
        .find(|value| (value - target).abs() / target < 1e-3)
}

/// Resuelve el reposo del inversor y las impedancias de fuente relevantes:
/// `(reposo, salida de placa, salida de cátodo, α de la placa del triodo)`.
fn solve_operating_point(
    pentode: &KorenPentodeModel,
    triode: &KorenTriodeModel,
    config: &PhaseInverter7199Config,
) -> Option<(PhaseInverterOperatingPoint, f64, f64, f64)> {
    // --- Pentodo (rejilla a masa por la fuga; Vgk = -Vk) ---
    let supply = config.preamp_supply_v;
    let feedback_conductance = config
        .feedback_resistor_ohms
        .map(|ohms| 1.0 / ohms.max(1.0))
        .unwrap_or(0.0);
    let mut screen = supply * 0.15;
    let mut cathode = 1.0;
    for _ in 0..2000 {
        let vgk = -cathode;
        // El modelo Koren usa tensiones referidas al cátodo.
        let screen_current = pentode.screen_current(screen - cathode, vgk);
        let screen_next = (supply / config.pentode_screen_feed_ohms - screen_current)
            / (1.0 / config.pentode_screen_feed_ohms + 1.0 / config.pentode_screen_ground_ohms);
        let plate_current = implicit_plate_current(pentode, config, screen - cathode, vgk, cathode);
        // Con NFB, la corriente de la 22 kΩ (secundario a 0 V en DC) sale del
        // cátodo: (Ip+Ig2) = vk/Rk + vk/Rf.
        let cathode_next = (plate_current + screen_current)
            / (1.0 / config.pentode_cathode_ohms + feedback_conductance);
        let delta = (screen_next - screen).abs() + (cathode_next - cathode).abs();
        screen += 0.3 * (screen_next - screen);
        cathode += 0.3 * (cathode_next - cathode);
        if delta < 1e-10 {
            break;
        }
    }
    let pentode_vgk = -cathode;
    let pentode_plate_v = supply
        - config.pentode_plate_ohms
            * implicit_plate_current(pentode, config, screen - cathode, pentode_vgk, cathode);

    // --- Triodo (malla 10 MΩ placa-rejilla, 3.3 MΩ rejilla-masa) ---
    let triode_supply = config.triode_supply_v;
    let plate_load = config.triode_plate_load_ohms;
    let cathode_load = config.triode_cathode_load_ohms;
    let ratio =
        config.grid_leak_ground_ohms / (config.grid_leak_plate_ohms + config.grid_leak_ground_ohms);
    // La 10 MΩ carga la placa: `Vp = (Bt − Ra·I)/(1 + (Ra/R10)(1 − ratio))`.
    let plate_divisor = 1.0 + plate_load * (1.0 - ratio) / config.grid_leak_plate_ohms.max(1.0);
    let mut low = 0.0;
    let mut high = triode_supply / (plate_load + cathode_load);
    for _ in 0..200 {
        let current = 0.5 * (low + high);
        let plate_v = (triode_supply - plate_load * current) / plate_divisor;
        let cathode_v = cathode_load * current;
        let grid_v = plate_v * ratio;
        let model = triode.plate_current(plate_v - cathode_v, grid_v - cathode_v);
        if model - current > 0.0 {
            low = current;
        } else {
            high = current;
        }
    }
    let current = 0.5 * (low + high);
    let triode_plate_v = (triode_supply - plate_load * current) / plate_divisor;
    let triode_cathode_v = cathode_load * current;
    let triode_grid_v = triode_plate_v * ratio;

    // Salidas del triodo: placa (rp || 47 kΩ) y cátodo (1/gm || 47 kΩ).
    let step = 0.5;
    let triode_plate_cathode = triode_plate_v - triode_cathode_v;
    let triode_slope = (triode.plate_current(
        triode_plate_cathode + step,
        triode_grid_v - triode_cathode_v,
    ) - triode.plate_current(
        triode_plate_cathode - step,
        triode_grid_v - triode_cathode_v,
    )) / (2.0 * step);
    let triode_rp = 1.0 / triode_slope.max(1e-12);
    let gm = (triode.plate_current(
        triode_plate_cathode,
        triode_grid_v - triode_cathode_v + 1e-4,
    ) - triode.plate_current(
        triode_plate_cathode,
        triode_grid_v - triode_cathode_v - 1e-4,
    )) / 2.0e-4;
    let upper = 1.0 / (1.0 / triode_rp.max(1.0) + 1.0 / plate_load);
    let lower = 1.0 / (gm.max(1e-9) + 1.0 / cathode_load);

    // Ganancia de placa del split-load por volt de rejilla (invertida):
    // α = gm·R / (1 + gm·R + 2R/rp), con R la carga de placa/cátodo.
    let alpha = gm * plate_load / (1.0 + gm * plate_load + 2.0 * plate_load / triode_rp.max(1.0));

    Some((
        PhaseInverterOperatingPoint {
            pentode_plate_v,
            pentode_screen_v: screen,
            pentode_cathode_v: cathode,
            triode_plate_v,
            triode_cathode_v,
            triode_grid_v,
        },
        upper,
        lower,
        alpha,
    ))
}

fn implicit_plate_current(
    pentode: &KorenPentodeModel,
    config: &PhaseInverter7199Config,
    screen_cathode_v: f64,
    grid_cathode_v: f64,
    cathode_v: f64,
) -> f64 {
    let supply = config.preamp_supply_v;
    let plate_resistor = config.pentode_plate_ohms;
    let residual = |current: f64| {
        pentode.plate_current(
            supply - plate_resistor * current - cathode_v,
            screen_cathode_v,
            grid_cathode_v,
        ) - current
    };
    let mut low = 0.0;
    let mut high = supply / plate_resistor;
    for _ in 0..80 {
        let middle = 0.5 * (low + high);
        if residual(middle) > 0.0 {
            low = middle;
        } else {
            high = middle;
        }
    }
    0.5 * (low + high)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::test_circuit;
    use std::f64::consts::PI;

    fn inverter() -> PhaseInverter7199 {
        PhaseInverter7199::from_circuit(&test_circuit(), 48_000.0).expect("datos trazados")
    }

    #[test]
    fn operating_point_matches_traced_network() {
        let stage = inverter();
        let op = stage.operating_point();
        // Contraste con el manual y con el golden SPICE (documentado).
        assert!(
            (op.triode_plate_v - 356.0).abs() / 356.0 < 0.05,
            "placa del triodo {} V",
            op.triode_plate_v
        );
        assert!(
            (op.triode_cathode_v - 102.7).abs() / 102.7 < 0.08,
            "cátodo del triodo {} V",
            op.triode_cathode_v
        );
        assert!(
            (op.pentode_plate_v - 85.2).abs() / 85.2 < 0.20,
            "placa del pentodo {} V",
            op.pentode_plate_v
        );
        assert!(
            (op.pentode_screen_v - 64.6).abs() / 64.6 < 0.15,
            "pantalla {} V",
            op.pentode_screen_v
        );
        assert!(
            (op.pentode_cathode_v - 1.87).abs() / 1.87 < 0.20,
            "cátodo del pentodo {} V",
            op.pentode_cathode_v
        );
    }

    #[test]
    fn silence_keeps_outputs_at_rest() {
        let mut stage = inverter();
        let mut worst: f64 = 0.0;
        for _ in 0..4096 {
            let (upper, lower) = stage.process(0.0, 0.0);
            worst = worst.max(upper.abs()).max(lower.abs());
        }
        assert!(worst < 1e-7, "residuo de silencio del inversor: {worst}");
        assert_eq!(stage.convergence_failures(), 0);
    }

    #[test]
    fn outputs_are_opposite_and_reasonably_balanced() {
        let mut stage = inverter();
        let sample_rate = 48_000.0;
        let mut upper_peak: f64 = 0.0;
        let mut lower_peak: f64 = 0.0;
        let mut same_sign = 0_u32;
        let frames = 24_000;
        for index in 0..frames {
            let input = 0.1 * (2.0 * PI * 1_000.0 * index as f64 / sample_rate).sin();
            let (upper, lower) = stage.process(input, 0.0);
            if index > frames / 2 {
                upper_peak = upper_peak.max(upper.abs());
                lower_peak = lower_peak.max(lower.abs());
                if upper * lower > 0.0 {
                    same_sign += 1;
                }
            }
        }
        assert!(
            upper_peak > 1.0,
            "salida de placa demasiado baja: {upper_peak}"
        );
        assert!(
            (upper_peak - lower_peak).abs() / upper_peak < 0.35,
            "desbalance split-load: placa {upper_peak}, cátodo {lower_peak}"
        );
        assert_eq!(same_sign, 0, "las dos salidas deben ser opuestas");
    }
}
