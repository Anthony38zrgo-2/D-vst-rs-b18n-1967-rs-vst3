//! B18-009 research candidate, NOT connected to the production engine.
//!
//! Simultaneous nodal solve of 7199, 7027A, OT and cathode NFB.
//! Blocked by independent negative loop margins and reactive transients;
//! see `docs/B18_009_STATUS.md`. AC agreement is not a stability certificate.
//!
//! Eleven node voltages, analytic Jacobian, bounded Newton with line search.
//! Every capacitor is stamped once; histories advance only on convergence.
//! The mixer is a Thevenin source with R = Rmix/2 (tone outputs idealized).
use crate::circuit::CircuitDefinition;
use crate::numerics::discretization::TptCapacitor;
use crate::numerics::nonlinear::{solve_with_jacobian, NewtonConfig, NewtonFailure};
use crate::stages::output_transformer::OutputTransformerModel;
use crate::stages::phase_splitter::{PhaseInverter7199, PhaseInverter7199Config};
use crate::stages::power_7027a::{PowerStageConfig, PushPullStage};
use crate::stages::power_supply::{SupplyLoad, SupplyRails};
use crate::stages::speaker_impedance::ElectricalLoadConfig;
use crate::stages::tube_models::{KorenPentodeModel, KorenTriodeModel};

const N: usize = 11;
const G: usize = 0;
const P: usize = 1;
const S: usize = 2;
const K: usize = 3;
const TG: usize = 4;
const TP: usize = 5;
const TK: usize = 6;
const U: usize = 7;
const L: usize = 8;
const A: usize = 9;
const B: usize = 10;
const GROUND: usize = N;
const NEWTON: NewtonConfig = NewtonConfig {
    max_iterations: 120,
    tolerance: 1e-6,
    residual_tolerance: 1e-5,
    max_step: 80.0,
    max_backtracks: 16,
};

#[derive(Clone, Copy, Debug)]
struct CapacitorBranch {
    a: usize,
    b: usize,
    series: f64,
    cap: TptCapacitor,
    conductance: f64,
    history_scale: f64,
}

impl CapacitorBranch {
    fn new(a: usize, b: usize, c: f64, series: f64, rate: f64) -> Self {
        let cap = TptCapacitor::new(c, 1.0 / rate);
        let history_scale = 1.0 / (1.0 + series * cap.conductance());
        Self {
            a,
            b,
            series,
            conductance: cap.conductance() * history_scale,
            history_scale,
            cap,
        }
    }
    fn voltage(&self, x: &[f64]) -> f64 {
        x[self.a] - if self.b == GROUND { 0.0 } else { x[self.b] }
    }
    fn port(&self) -> (f64, f64) {
        (
            self.conductance,
            self.cap.companion_current() * self.history_scale,
        )
    }
    fn advance(&mut self, x: &[f64]) {
        let v = self.voltage(x);
        let (g, history) = self.port();
        self.cap.update(v - self.series * (g * v - history));
    }
}

/// Coupled output circuit; all working storage has fixed size.
#[derive(Clone, Debug)]
pub struct OutputNetwork {
    inverter: PhaseInverter7199Config,
    power: PowerStageConfig,
    pentode: KorenPentodeModel,
    triode: KorenTriodeModel,
    beam: KorenPentodeModel,
    transformer: OutputTransformerModel,
    caps: [CapacitorBranch; 16],
    nodes: [f64; N],
    mixer_ohms: f64,
    tap_gain: f64,
    failures: u64,
    last_failure: Option<NewtonFailure>,
    plate_currents: [f64; 2],
    linear_matrix: [[f64; N]; N],
}

impl OutputNetwork {
    /// Solve all eleven node voltages with analytic Jacobian. The 7199 pentode
    /// grid-conduction branch makes the mixer-grid (`G`) row nonlinear, so the
    /// node is not eliminated and all eleven nodes are solved directly.
    fn solve_nodes(&mut self, x: &mut [f64; N], mixed: f64, dc: bool) -> Option<[f64; 2]> {
        let (offset, matrix) = self.linear_system(mixed, dc);
        let mut plate_currents = [0.0; 2];
        let report = solve_with_jacobian::<N, _>(
            x,
            |state, r, j| {
                plate_currents = self.evaluate(state, &offset, &matrix, r, j);
            },
            &NEWTON,
        );
        self.record(&report);
        report.converged.then_some(plate_currents)
    }

    fn record(&mut self, report: &crate::numerics::nonlinear::NewtonReport) {
        self.last_failure = if report.converged {
            None
        } else {
            report.failure
        };
    }

    /// Kind of the last Newton failure (diagnostic; `None` after convergence).
    #[must_use]
    pub fn last_failure(&self) -> Option<NewtonFailure> {
        self.last_failure
    }
    /// Seed from already solved, supply-consistent DC operating points.
    pub fn new(
        circuit: &CircuitDefinition,
        inverter: &PhaseInverter7199,
        power: &PushPullStage,
    ) -> Option<Self> {
        let capacitances = circuit.tube_models.capacitances?;
        let config = *inverter.config();
        let pc = *power.config();
        let rate = config.sample_rate_hz;
        let branch = |a, b, c| CapacitorBranch::new(a, b, c, 0.0, rate);
        let cp = capacitances.pentode_7199;
        let ct = capacitances.triode_7199;
        let cb = capacitances.power_7027a;
        let mut network = Self {
            inverter: config,
            power: pc,
            pentode: KorenPentodeModel::new(circuit.tube_models.koren_7199_pentode?),
            triode: KorenTriodeModel::new(circuit.tube_models.koren_7199_triode?),
            beam: KorenPentodeModel::new(circuit.tube_models.koren_7027a?),
            transformer: OutputTransformerModel::new(pc.transformer, 1.0 / rate).ok()?,
            caps: [
                branch(S, K, config.screen_bypass_farads),
                branch(P, TG, config.coupling_farads),
                CapacitorBranch::new(TP, U, pc.coupling_farads, pc.grid_stopper_ohms, rate),
                CapacitorBranch::new(TK, L, pc.coupling_farads, pc.grid_stopper_ohms, rate),
                branch(G, P, cp.gp),
                branch(G, K, cp.gk),
                branch(P, K, cp.pk),
                branch(TG, TP, ct.gp),
                branch(TG, TK, ct.gk),
                branch(TP, TK, ct.pk),
                branch(U, A, cb.gp),
                branch(U, GROUND, cb.gk),
                branch(A, GROUND, cb.pk),
                branch(L, B, cb.gp),
                branch(L, GROUND, cb.gk),
                branch(B, GROUND, cb.pk),
            ],
            nodes: [0.0; N],
            mixer_ohms: circuit.channel.channel_mixer_resistor_ohms / 2.0,
            tap_gain: -pc.transformer.turns_ratio_8ohm / pc.transformer.turns_ratio_16ohm,
            failures: 0,
            last_failure: None,
            plate_currents: [0.0; 2],
            linear_matrix: [[0.0; N]; N],
        };
        let op = inverter.operating_point();
        network.nodes = [
            0.0,
            op.pentode_plate_v,
            op.pentode_screen_v,
            op.pentode_cathode_v,
            op.triode_grid_v,
            op.triode_plate_v,
            op.triode_cathode_v,
            pc.bias_v,
            pc.bias_v,
            power.plate_voltage(0),
            power.plate_voltage(1),
        ];
        // Re-solve DC with all capacitors open. No transient is used to find DC.
        let mut x = network.nodes;
        network.plate_currents = network.solve_nodes(&mut x, 0.0, true)?;
        network.nodes = x;
        for cap in &mut network.caps {
            cap.cap.preset_voltage(cap.voltage(&x));
        }
        network.linear_matrix = network.linear_system_full(0.0, false).1;
        Some(network)
    }

    pub fn set_rails(&mut self, rails: SupplyRails) {
        self.inverter.preamp_supply_v = rails.preamp_v;
        self.inverter.triode_supply_v = rails.triode_v;
        self.power.supply_v = rails.bplus_ct_v;
        self.power.screen_supply_v = rails.screen_v;
        self.power.bias_v = rails.bias_v;
    }

    pub fn load_config(&self) -> ElectricalLoadConfig {
        self.transformer.load_config()
    }
    pub fn set_load(&mut self, load: ElectricalLoadConfig) {
        self.transformer.set_load(load);
        self.linear_matrix = self.linear_system_full(0.0, false).1;
    }
    pub fn convergence_failures(&self) -> u64 {
        self.failures
    }

    /// Diagnostic node order: G, P, S, K, TG, TP, TK, U, L, A, B.
    #[must_use]
    pub fn node_voltages(&self) -> [f64; N] {
        self.nodes
    }

    pub fn supply_load(&self) -> SupplyLoad {
        let x = &self.nodes;
        let screen =
            |index: usize, plate: usize, grid: usize| self.screen_current_for(index, plate, grid);
        SupplyLoad {
            ct_a: (2.0 * self.power.supply_v - x[A] - x[B])
                / (0.5 * self.power.transformer.dc_primary_winding_ohms),
            screen_a: screen(0, A, U) + screen(1, B, L),
            triode_a: (self.inverter.triode_supply_v - x[TP])
                / self.inverter.triode_plate_load_ohms,
            preamp_a: (self.inverter.preamp_supply_v - x[P]) / self.inverter.pentode_plate_ohms
                + (self.inverter.preamp_supply_v - x[S]) / self.inverter.pentode_screen_feed_ohms,
            bias_a: (2.0 * self.power.bias_v - x[U] - x[L]) / self.power.grid_return_ohms,
        }
    }

    /// Screen current of one 7027A from the current plate solution (B18-011).
    fn screen_current_for(&self, index: usize, plate: usize, grid: usize) -> f64 {
        let x = &self.nodes;
        let params = self.beam.params();
        if params.screen_ratio_floor > 0.0 {
            self.plate_currents[index]
                * (params.screen_ratio_floor
                    + params.screen_ratio_boost
                        * (-x[plate].max(0.0) / params.screen_ratio_volts.max(1e-6)).exp())
        } else {
            self.beam
                .screen_current(x[plate], self.power.screen_supply_v, x[grid])
        }
    }

    /// Per-tube plate currents of the last converged sample (diagnostic).
    #[must_use]
    pub fn plate_currents(&self) -> [f64; 2] {
        self.plate_currents
    }

    /// Per-tube screen currents of the last converged sample (diagnostic).
    #[must_use]
    pub fn screen_currents(&self) -> [f64; 2] {
        [
            self.screen_current_for(0, A, U),
            self.screen_current_for(1, B, L),
        ]
    }

    /// Mean idle plate current per 7027A (diagnostic operating point).
    #[must_use]
    pub fn idle_plate_current_a(&self) -> f64 {
        0.5 * (self.plate_currents[0] + self.plate_currents[1])
    }

    /// Mean idle screen current per 7027A (diagnostic operating point).
    #[must_use]
    pub fn idle_screen_current_a(&self) -> f64 {
        let currents = self.screen_currents();
        0.5 * (currents[0] + currents[1])
    }

    /// 7199 operating point from the last solved node voltages.
    #[must_use]
    pub fn inverter_operating_point(
        &self,
    ) -> crate::stages::phase_splitter::PhaseInverterOperatingPoint {
        crate::stages::phase_splitter::PhaseInverterOperatingPoint {
            pentode_plate_v: self.nodes[P],
            pentode_screen_v: self.nodes[S],
            pentode_cathode_v: self.nodes[K],
            triode_plate_v: self.nodes[TP],
            triode_cathode_v: self.nodes[TK],
            triode_grid_v: self.nodes[TG],
        }
    }

    /// Plate voltage of the first 7027A (node A) from the last solved sample.
    #[must_use]
    pub fn power_plate_voltage(&self) -> f64 {
        self.nodes[A]
    }

    /// Returns the last valid voltage on failure; commits no partial histories.
    pub fn process(&mut self, mixed: f64) -> f64 {
        let mut x = self.nodes;
        let Some(currents) = self.solve_nodes(&mut x, mixed, false) else {
            self.failures += 1;
            return self.transformer.secondary_v();
        };
        if !x.iter().all(|v| v.is_finite()) {
            self.failures += 1;
            return self.transformer.secondary_v();
        }
        self.plate_currents = currents;
        let difference = x[A] - x[B];
        let current = self.transformer.differential_current(difference);
        self.transformer.advance(difference, current);
        for cap in &mut self.caps {
            cap.advance(&x);
        }
        self.nodes = x;
        self.transformer.secondary_v()
    }

    fn linear_system(&self, mixed: f64, dc: bool) -> ([f64; N], [[f64; N]; N]) {
        if dc {
            return self.linear_system_full(mixed, true);
        }
        let c = self.inverter;
        let mut offset = [0.0; N];
        offset[G] = -mixed / self.mixer_ohms;
        offset[P] = -c.preamp_supply_v / c.pentode_plate_ohms;
        offset[S] = -c.preamp_supply_v / c.pentode_screen_feed_ohms;
        offset[TP] = -c.triode_supply_v / c.triode_plate_load_ohms;
        offset[U] = -self.power.bias_v / self.power.grid_return_ohms;
        offset[L] = offset[U];
        for cap in &self.caps {
            let history = cap.cap.companion_current() * cap.history_scale;
            offset[cap.a] -= history;
            if cap.b != GROUND {
                offset[cap.b] += history;
            }
        }
        let (history, slope) = self.transformer.differential_drop(0.0);
        let common = -2.0 * self.power.supply_v / self.power.transformer.dc_primary_winding_ohms;
        offset[A] += common + history / slope;
        offset[B] += common - history / slope;
        if let Some(resistance) = c.feedback_resistor_ohms {
            offset[K] -= self.tap_gain * self.transformer.secondary_voltage_port().0 / resistance;
        }
        (offset, self.linear_matrix)
    }

    fn linear_system_full(&self, mixed: f64, dc: bool) -> ([f64; N], [[f64; N]; N]) {
        let x = &[0.0; N];
        let mut offset = [0.0; N];
        let mut matrix = [[0.0; N]; N];
        let r = &mut offset[..];
        let j = &mut matrix;
        r.fill(0.0);
        j.fill([0.0; N]);
        let c = self.inverter;
        // Linear branches, including actual loading of the split-load outputs.
        for (node, voltage, resistance) in [
            (G, mixed, self.mixer_ohms),
            (P, c.preamp_supply_v, c.pentode_plate_ohms),
            (S, c.preamp_supply_v, c.pentode_screen_feed_ohms),
            (S, 0.0, c.pentode_screen_ground_ohms),
            (K, 0.0, c.pentode_cathode_ohms),
            (TG, 0.0, c.grid_leak_ground_ohms),
            (TP, c.triode_supply_v, c.triode_plate_load_ohms),
            (TK, 0.0, c.triode_cathode_load_ohms),
            (U, self.power.bias_v, self.power.grid_return_ohms),
            (L, self.power.bias_v, self.power.grid_return_ohms),
        ] {
            let g = 1.0 / resistance;
            r[node] += g * (x[node] - voltage);
            j[node][node] += g;
        }
        stamp(x, r, j, TP, TG, 1.0 / c.grid_leak_plate_ohms, 0.0);
        if !dc {
            for cap in &self.caps {
                let (g, h) = cap.port();
                stamp(x, r, j, cap.a, cap.b, g, h);
            }
        }
        // The OT port is affine for fixed histories. Its prediction is read-only.
        let difference = x[A] - x[B];
        let (history, slope) = self.transformer.differential_drop(0.0);
        let (history, slope) = if dc {
            (0.0, self.power.transformer.dc_primary_winding_ohms)
        } else {
            (history, slope)
        };
        let common_g = 1.0 / self.power.transformer.dc_primary_winding_ohms;
        let common = common_g * (x[A] + x[B] - 2.0 * self.power.supply_v);
        let differential = (difference + history) / slope;
        r[A] += common + differential;
        r[B] += common - differential;
        j[A][A] += common_g + 1.0 / slope;
        j[A][B] += common_g - 1.0 / slope;
        j[B][A] += common_g - 1.0 / slope;
        j[B][B] += common_g + 1.0 / slope;
        if let Some(resistance) = c.feedback_resistor_ohms {
            let (v, dv) = if dc {
                (0.0, 0.0)
            } else {
                let (offset, gain) = self.transformer.secondary_voltage_port();
                (
                    self.tap_gain * (offset + gain * difference),
                    self.tap_gain * gain,
                )
            };
            r[K] += (x[K] - v) / resistance;
            j[K][K] += 1.0 / resistance;
            j[K][A] -= dv / resistance;
            j[K][B] += dv / resistance;
        }

        (offset, matrix)
    }
    fn evaluate(
        &self,
        x: &[f64],
        offset: &[f64; N],
        matrix: &[[f64; N]; N],
        r: &mut [f64],
        j: &mut [[f64; N]; N],
    ) -> [f64; 2] {
        *j = *matrix;
        for row in 0..N {
            r[row] = offset[row];
            for (col, voltage) in x.iter().enumerate() {
                r[row] += matrix[row][col] * voltage;
            }
        }
        let p = self
            .pentode
            .currents_with_slopes(x[P] - x[K], x[S] - x[K], x[G] - x[K]);
        for (row, current, dp, ds, dg) in [
            (
                P,
                p.plate_a,
                p.d_plate_d_plate,
                p.d_plate_d_screen,
                p.d_plate_d_grid,
            ),
            (
                S,
                p.screen_a,
                p.d_screen_d_plate,
                p.d_screen_d_screen,
                p.d_screen_d_grid,
            ),
        ] {
            r[row] += current;
            r[K] -= current;
            for (col, d) in [(P, dp), (S, ds), (G, dg), (K, -dp - ds - dg)] {
                j[row][col] += d;
                j[K][col] -= d;
            }
        }
        let (ip, dp, dg) = self
            .triode
            .plate_current_with_slopes(x[TP] - x[TK], x[TG] - x[TK]);
        r[TP] += ip;
        r[TK] -= ip;
        for (col, d) in [(TP, dp), (TG, dg), (TK, -dp - dg)] {
            j[TP][col] += d;
            j[TK][col] -= d;
        }
        // 7199 pentode grid conduction (clamps the mixer grid under strong
        // drive); previously the declared unmodelled branch.
        let (ig_pentode, dig_pentode) = self.pentode.grid_current_with_slope(x[G] - x[K]);
        stamp_current(r, j, G, K, ig_pentode, dig_pentode);
        let (ig, dig) = self.triode.grid_current_with_slope(x[TG] - x[TK]);
        stamp_current(r, j, TG, TK, ig, dig);
        let mut plate_currents = [0.0; 2];
        for (index, (grid, plate)) in [(U, A), (L, B)].into_iter().enumerate() {
            let (ip, dp, dg) =
                self.beam
                    .plate_grid_slopes(x[plate], self.power.screen_supply_v, x[grid]);
            plate_currents[index] = ip;
            r[plate] += ip;
            j[plate][plate] += dp;
            j[plate][grid] += dg;
            let (ig, dig) = self.beam.grid_current_with_slope(x[grid]);
            r[grid] += ig;
            j[grid][grid] += dig;
        }
        // Fixed row equilibration: measure residuals in volts at each node,
        // so high-current plates do not hide an unresolved high-impedance grid.
        for (row, scale) in [
            135e3, 270e3, 100e3, 1500.0, 100e3, 47e3, 47e3, 100e3, 100e3, 1000.0, 1000.0,
        ]
        .iter()
        .enumerate()
        {
            r[row] *= scale;
            for value in &mut j[row] {
                *value *= scale;
            }
        }
        plate_currents
    }
}

fn stamp(
    x: &[f64],
    r: &mut [f64],
    j: &mut [[f64; N]; N],
    a: usize,
    b: usize,
    g: f64,
    history: f64,
) {
    let v = x[a] - if b == GROUND { 0.0 } else { x[b] };
    let current = g * v - history;
    r[a] += current;
    j[a][a] += g;
    if b != GROUND {
        r[b] -= current;
        j[a][b] -= g;
        j[b][a] -= g;
        j[b][b] += g;
    }
}
fn stamp_current(
    r: &mut [f64],
    j: &mut [[f64; N]; N],
    a: usize,
    b: usize,
    current: f64,
    slope: f64,
) {
    r[a] += current;
    r[b] -= current;
    j[a][a] += slope;
    j[a][b] -= slope;
    j[b][a] -= slope;
    j[b][b] += slope;
}
