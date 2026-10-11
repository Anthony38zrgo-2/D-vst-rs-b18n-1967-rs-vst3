//! Modelos de válvula.
//!
//! Hito C (parte 1): triodo con las ecuaciones de Norman Koren.
//!
//! ```text
//! E1 = (Vp/Kp)·ln(1 + exp(Kp·(1/µ + Vg_ef/sqrt(Kvb + Vp²))))
//! Ip = E1^Ex / Kg1      (E1 > 0; Ip = 0 en caso contrario)
//! ```
//!
//! Las ecuaciones son públicas (Norman Koren, *Improved vacuum tube models
//! for SPICE*). Los parámetros **no** están en el JSON canónico: llegan del
//! overlay de calibración con su procedencia (`estimated`/`measured`/
//! `datasheet`). No caracterizan la conducción de rejilla ni la saturación
//! real de pantalla: eso llega con la 7027A (parte 2 del Hito C).

use crate::circuit::{KorenPentodeParameters, KorenTriodeParameters};

/// Corrientes y derivadas analíticas de un pentodo referidas al cátodo
/// (`Ola 8`, para el Newton 3D del inversor 7199 sin jacobiano numérico).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PentodeCurrents {
    pub plate_a: f64,
    pub screen_a: f64,
    /// `∂Ip/∂Vpk`.
    pub d_plate_d_plate: f64,
    /// `∂Ip/∂Vsk`.
    pub d_plate_d_screen: f64,
    /// `∂Ip/∂Vgk`.
    pub d_plate_d_grid: f64,
    /// `∂Ig2/∂Vsk`.
    pub d_screen_d_screen: f64,
    /// `∂Ig2/∂Vgk`.
    pub d_screen_d_grid: f64,
}

impl PentodeCurrents {
    /// Nulo (regiones de corte).
    pub const ZERO: Self = Self {
        plate_a: 0.0,
        screen_a: 0.0,
        d_plate_d_plate: 0.0,
        d_plate_d_screen: 0.0,
        d_plate_d_grid: 0.0,
        d_screen_d_screen: 0.0,
        d_screen_d_grid: 0.0,
    };
}

/// Triodo Koren.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KorenTriodeModel {
    params: KorenTriodeParameters,
}

impl KorenTriodeModel {
    #[must_use]
    pub fn new(params: KorenTriodeParameters) -> Self {
        Self { params }
    }

    #[must_use]
    pub fn params(&self) -> KorenTriodeParameters {
        self.params
    }

    /// Corriente de placa (A) para tensión de placa y `Vgk`.
    ///
    /// Función continua, sin `NaN`: la exponencial se satura por tramos y una
    /// `E1` no positiva da corriente cero (zona de corte).
    #[must_use]
    pub fn plate_current(&self, plate_v: f64, grid_cathode_v: f64) -> f64 {
        self.plate_current_with_slopes(plate_v, grid_cathode_v).0
    }

    /// Corriente de placa y pendientes analíticas `(Ip, ∂Ip/∂Vp, ∂Ip/∂Vgk)`.
    ///
    /// El jacobiano analítico es lo que mantiene la convergencia de Newton con
    /// señales fuertes (la rejilla entra en la zona exponencial).
    #[must_use]
    pub fn plate_current_with_slopes(&self, plate_v: f64, grid_cathode_v: f64) -> (f64, f64, f64) {
        let clamped_plate = plate_v <= 0.0;
        let vp = plate_v.max(0.0);
        let root = (self.params.kvb + vp * vp).sqrt();
        if root <= 0.0 || clamped_plate {
            return (0.0, 0.0, 0.0);
        }
        let inner =
            self.params.kp * (1.0 / self.params.mu + (grid_cathode_v + self.params.vct) / root);
        // Una sola exponencial alimenta softplus y sigmoide (Ola 8).
        let (softplus, sigmoid) = if inner > 40.0 {
            (inner, 1.0)
        } else if inner < -40.0 {
            (0.0, 0.0)
        } else {
            let z = inner.exp();
            (z.ln_1p(), z / (1.0 + z))
        };
        let e1 = vp / self.params.kp * softplus;
        if e1 <= 0.0 {
            return (0.0, 0.0, 0.0);
        }
        let current = e1.powf(self.params.ex) / self.params.kg1;
        // ∂Ip/∂E1 = Ex·Ip/E1 (evita el segundo `powf`).
        let d_current_de1 = self.params.ex * current / e1;

        let denominator = self.params.kvb + vp * vp;
        let d_inner_dvp =
            self.params.kp * (grid_cathode_v + self.params.vct) * (-vp / denominator.powf(1.5));
        let d_inner_dvgk = self.params.kp / root;

        let d_e1_dvp = softplus / self.params.kp + vp / self.params.kp * sigmoid * d_inner_dvp;
        let d_e1_dvgk = vp / self.params.kp * sigmoid * d_inner_dvgk;

        (current, d_current_de1 * d_e1_dvp, d_current_de1 * d_e1_dvgk)
    }

    /// Corriente de rejilla (A) y su pendiente respecto de `Vgk`.
    ///
    /// Aproximación documentada en la calibración: conducción con transición
    /// suave (softplus) a partir de `grid_conduction_threshold_v`, con
    /// resistencia equivalente `grid_conduction_resistance_ohms`. La
    /// transición suave evita la discontinuidad de derivada que bloquea a
    /// Newton justo en el umbral. No sustituye a una curva de rejilla medida.
    #[must_use]
    pub fn grid_current_with_slope(&self, grid_cathode_v: f64) -> (f64, f64) {
        const SOFTNESS_V: f64 = 0.08;
        let resistance = self.params.grid_conduction_resistance_ohms.max(1.0);
        let shifted = (grid_cathode_v - self.params.grid_conduction_threshold_v) / SOFTNESS_V;
        if shifted > 40.0 {
            (
                (grid_cathode_v - self.params.grid_conduction_threshold_v) / resistance,
                1.0 / resistance,
            )
        } else if shifted < -40.0 {
            (0.0, 0.0)
        } else {
            // Una sola exponencial alimenta softplus y sigmoide (Ola 8).
            let z = shifted.exp();
            (
                SOFTNESS_V * z.ln_1p() / resistance,
                (z / (1.0 + z)) / resistance,
            )
        }
    }

    /// Transconductancia de pequeña señal en un punto (derivada numérica).
    #[must_use]
    pub fn transconductance(&self, plate_v: f64, grid_cathode_v: f64) -> f64 {
        let step = 1e-4;
        (self.plate_current(plate_v, grid_cathode_v + step)
            - self.plate_current(plate_v, grid_cathode_v - step))
            / (2.0 * step)
    }

    /// Resistencia de placa de pequeña señal en un punto (derivada numérica).
    #[must_use]
    pub fn plate_resistance(&self, plate_v: f64, grid_cathode_v: f64) -> f64 {
        let step = 1e-2;
        let slope = (self.plate_current(plate_v + step, grid_cathode_v)
            - self.plate_current(plate_v - step, grid_cathode_v))
            / (2.0 * step);
        1.0 / slope.max(1e-12)
    }
}

/// Pentodo/beam tetrode Koren (corriente de placa y de pantalla).
///
/// Sigue la forma publicada por Norman Koren:
///
/// ```text
/// E1  = (Vs/Kp)·ln(1 + exp(Kp·(1/µ + Vg/Vs)))          (Vs = pantalla-cátodo)
/// Ip  = 2·max(E1,0)^Ex / Kg1 · atan(Vp/Kvb)             (rodilla)
/// Ig2 = max(Vs/µ + Vg, 0)^Ex / Kg2
/// ```
///
/// El factor 2 proviene de la convención `PWR+PWRS` de la publicación, de
/// modo que los parámetros publicados (p. ej. 6L6GC) son directamente
/// utilizables. El atan modela la rodilla de placa.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KorenPentodeModel {
    params: KorenPentodeParameters,
}

impl KorenPentodeModel {
    #[must_use]
    pub fn new(params: KorenPentodeParameters) -> Self {
        Self { params }
    }

    #[must_use]
    pub fn params(&self) -> KorenPentodeParameters {
        self.params
    }

    /// Corriente de placa (A) y pendiente `∂Ip/∂Vp`.
    #[must_use]
    pub fn plate_current_with_vp_slope(
        &self,
        plate_v: f64,
        screen_v: f64,
        grid_v: f64,
    ) -> (f64, f64) {
        if screen_v <= 1e-6 {
            return (0.0, 0.0);
        }
        let inner = self.params.kp * (1.0 / self.params.mu + (grid_v + self.params.vct) / screen_v);
        let softplus = if inner > 40.0 {
            inner
        } else if inner < -40.0 {
            0.0
        } else {
            (1.0 + inner.exp()).ln()
        };
        let e1 = screen_v / self.params.kp * softplus;
        if e1 <= 0.0 {
            return (0.0, 0.0);
        }
        let base = 2.0 * e1.powf(self.params.ex) / self.params.kg1;
        let knee_argument = plate_v.max(0.0) / self.params.kvb;
        let knee = knee_argument.atan();
        let slope = base * (1.0 / self.params.kvb) / (1.0 + knee_argument * knee_argument);
        (base * knee, slope)
    }

    /// Corriente de placa (A).
    #[must_use]
    pub fn plate_current(&self, plate_v: f64, screen_v: f64, grid_v: f64) -> f64 {
        self.plate_current_with_vp_slope(plate_v, screen_v, grid_v)
            .0
    }

    /// Corriente de pantalla (A).
    #[must_use]
    pub fn screen_current(&self, screen_v: f64, grid_v: f64) -> f64 {
        let base = screen_v / self.params.mu + grid_v + self.params.vct;
        if base <= 1e-9 {
            return 0.0;
        }
        base.powf(self.params.ex) / self.params.kg2
    }

    /// Corrientes de placa y pantalla con derivadas analíticas respecto de las
    /// tres tensiones referidas al cátodo (Ola 8, resolver el inversor acoplado
    /// con Newton 3D sin jacobiano numérico).
    ///
    /// Las pendientes cubren las regiones de corte con las mismas guardas que
    /// [`Self::plate_current_with_vp_slope`] y [`Self::screen_current`].
    #[must_use]
    pub fn currents_with_slopes(
        &self,
        plate_cathode_v: f64,
        screen_cathode_v: f64,
        grid_cathode_v: f64,
    ) -> PentodeCurrents {
        let screen_k = screen_cathode_v;
        let plate_k = plate_cathode_v.max(0.0);
        let grid_k = grid_cathode_v;
        if screen_k <= 1e-6 {
            return PentodeCurrents::ZERO;
        }

        // Pantalla: Ig2 = (sk/µ + gk + vct)^Ex / Kg2.
        let base = screen_k / self.params.mu + grid_k + self.params.vct;
        let (screen_a, d_screen_d_screen, d_screen_d_grid) = if base <= 1e-9 {
            (0.0, 0.0, 0.0)
        } else {
            // Un solo `powf` alimenta corriente y pendiente (Ola 8).
            let power = base.powf(self.params.ex - 1.0);
            (
                power * base / self.params.kg2,
                self.params.ex * power / (self.params.kg2 * self.params.mu),
                self.params.ex * power / self.params.kg2,
            )
        };

        // Placa: Ip = 2·E1^Ex/Kg1·atan(Vpk/Kvb), E1 = (sk/Kp)·softplus(Kp(1/µ + gk/sk)).
        let inner = self.params.kp * (1.0 / self.params.mu + grid_k / screen_k);
        // Una sola exponencial alimenta softplus y sigmoide (Ola 8).
        let (softplus, sigmoid) = if inner > 40.0 {
            (inner, 1.0)
        } else if inner < -40.0 {
            (0.0, 0.0)
        } else {
            let z = inner.exp();
            (z.ln_1p(), z / (1.0 + z))
        };
        let e1 = screen_k / self.params.kp * softplus;
        if e1 <= 0.0 {
            return PentodeCurrents {
                plate_a: 0.0,
                screen_a,
                d_plate_d_plate: 0.0,
                d_plate_d_screen: 0.0,
                d_plate_d_grid: 0.0,
                d_screen_d_screen,
                d_screen_d_grid,
            };
        }
        let weight = 2.0 * e1.powf(self.params.ex) / self.params.kg1;
        // ∂w/∂E1 = Ex·w/E1 (w = 2·E1^Ex/Kg1).
        let weight_slope = self.params.ex * weight / e1;
        // ∂E1/∂sk = softplus/Kp − (gk/sk)·sigmoid;  ∂E1/∂gk = sigmoid.
        let d_e1_d_screen = softplus / self.params.kp - grid_k / screen_k * sigmoid;
        let d_e1_d_grid = sigmoid;
        let knee_argument = plate_k / self.params.kvb;
        let knee = knee_argument.atan();
        let d_knee_d_plate = (1.0 / self.params.kvb) / (1.0 + knee_argument * knee_argument);

        PentodeCurrents {
            plate_a: weight * knee,
            screen_a,
            d_plate_d_plate: weight * d_knee_d_plate,
            d_plate_d_screen: weight_slope * d_e1_d_screen * knee,
            d_plate_d_grid: weight_slope * d_e1_d_grid * knee,
            d_screen_d_screen,
            d_screen_d_grid,
        }
    }

    /// Corriente de rejilla (A) y pendiente respecto de `Vgk` (misma
    /// aproximación suave que el triodo; parámetros de la calibración).
    #[must_use]
    pub fn grid_current_with_slope(&self, grid_cathode_v: f64) -> (f64, f64) {
        const SOFTNESS_V: f64 = 0.08;
        let resistance = self.params.grid_conduction_resistance_ohms.max(1.0);
        let shifted = (grid_cathode_v - self.params.grid_conduction_threshold_v) / SOFTNESS_V;
        if shifted > 40.0 {
            (
                (grid_cathode_v - self.params.grid_conduction_threshold_v) / resistance,
                1.0 / resistance,
            )
        } else if shifted < -40.0 {
            (0.0, 0.0)
        } else {
            let softplus = SOFTNESS_V * (1.0 + shifted.exp()).ln();
            let sigmoid = 1.0 / (1.0 + (-shifted).exp());
            (softplus / resistance, sigmoid / resistance)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> KorenTriodeModel {
        KorenTriodeModel::new(KorenTriodeParameters {
            mu: 66.0811,
            ex: 1.098449,
            kg1: 816.8934,
            kp: 592.0055,
            kvb: 300.3225,
            vct: 0.0,
            grid_conduction_threshold_v: -0.3,
            grid_conduction_resistance_ohms: 1_000.0,
        })
    }

    #[test]
    fn matches_fitted_anchors() {
        let model = model();
        let a1 = model.plate_current(175.0, -2.4);
        let a2 = model.plate_current(185.0, -2.0);
        let a3 = model.plate_current(250.0, -2.0);
        assert!((a1 - 0.4286e-3).abs() / 0.4286e-3 < 0.15, "A1 {a1}");
        assert!((a2 - 0.9091e-3).abs() / 0.9091e-3 < 0.15, "A2 {a2}");
        assert!((a3 - 2.3e-3).abs() / 2.3e-3 < 0.05, "A3 {a3}");
    }

    #[test]
    fn small_signal_targets_within_tolerance() {
        let model = model();
        let gm = model.transconductance(250.0, -2.0);
        let rp = model.plate_resistance(250.0, -2.0);
        assert!(gm > 1.2e-3 && gm < 2.0e-3, "gm {gm}");
        assert!(rp > 35e3 && rp < 60e3, "rp {rp}");
    }

    #[test]
    fn cut_off_and_guards_are_finite() {
        let model = model();
        assert_eq!(model.plate_current(250.0, -50.0), 0.0);
        assert_eq!(model.plate_current(-10.0, 0.0), 0.0);
        assert!(model.plate_current(0.0, 5.0).is_finite());
        assert!(model.plate_current(1e6, 5.0).is_finite());
        assert!(model.plate_current(250.0, 1e6).is_finite());
    }

    #[test]
    fn plate_current_increases_with_grid_voltage() {
        let model = model();
        let low = model.plate_current(200.0, -4.0);
        let mid = model.plate_current(200.0, -2.0);
        let high = model.plate_current(200.0, 0.0);
        assert!(low < mid && mid < high, "{low} {mid} {high}");
    }
}

#[cfg(test)]
mod pentode_tests {
    use super::*;

    fn model_6l6gc() -> KorenPentodeModel {
        KorenPentodeModel::new(KorenPentodeParameters {
            mu: 8.7,
            ex: 1.35,
            kg1: 1460.0,
            kg2: 4500.0,
            kp: 48.0,
            kvb: 12.0,
            vct: 0.0,
            grid_conduction_threshold_v: 0.0,
            grid_conduction_resistance_ohms: 500.0,
        })
    }

    #[test]
    fn matches_published_6l6gc_anchor() {
        // RCA 6L6GC, clase A típica: Ua=360 V, Ug2=250 V, Ug1=-18 V,
        // Ia ≈ 54 mA e Ig2 ≈ 5 mA (parámetros de Koren con esa misma base).
        let model = model_6l6gc();
        let ip = model.plate_current(360.0, 250.0, -18.0);
        let ig2 = model.screen_current(250.0, -18.0);
        assert!((ip - 0.054).abs() / 0.054 < 0.30, "Ip {ip}");
        assert!((ig2 - 0.0055).abs() / 0.0055 < 0.45, "Ig2 {ig2}");
    }

    #[test]
    fn analytic_slopes_match_finite_differences_and_values() {
        let model = model_6l6gc();
        let step = 1e-5;
        for (plate, screen, grid) in [
            (360.0, 250.0, -18.0),
            (85.0, 64.0, -1.8),
            (12.0, 90.0, -5.0),
            (250.0, 88.0, 0.5),
        ] {
            let currents = model.currents_with_slopes(plate, screen, grid);
            // Coherencia con los métodos existentes.
            assert!((currents.plate_a - model.plate_current(plate, screen, grid)).abs() < 1e-15);
            assert!((currents.screen_a - model.screen_current(screen, grid)).abs() < 1e-15);
            assert!(
                (currents.d_plate_d_plate
                    - model.plate_current_with_vp_slope(plate, screen, grid).1)
                    .abs()
                    < plate.abs().max(1.0) * 1e-9,
                "∂Ip/∂Vp en ({plate}, {screen}, {grid})"
            );
            // Diferencias finitas centrales.
            let fd = |forward: f64, backward: f64| (forward - backward) / (2.0 * step);
            let d_pp = fd(
                model.plate_current(plate + step, screen, grid),
                model.plate_current(plate - step, screen, grid),
            );
            let d_ps = fd(
                model.plate_current(plate, screen + step, grid),
                model.plate_current(plate, screen - step, grid),
            );
            let d_pg = fd(
                model.plate_current(plate, screen, grid + step),
                model.plate_current(plate, screen, grid - step),
            );
            let d_ss = fd(
                model.screen_current(screen + step, grid),
                model.screen_current(screen - step, grid),
            );
            let d_sg = fd(
                model.screen_current(screen, grid + step),
                model.screen_current(screen, grid - step),
            );
            for (name, analytic, numeric) in [
                ("∂Ip/∂Vp", currents.d_plate_d_plate, d_pp),
                ("∂Ip/∂Vs", currents.d_plate_d_screen, d_ps),
                ("∂Ip/∂Vg", currents.d_plate_d_grid, d_pg),
                ("∂Ig2/∂Vs", currents.d_screen_d_screen, d_ss),
                ("∂Ig2/∂Vg", currents.d_screen_d_grid, d_sg),
            ] {
                let scale = numeric.abs().max(1e-12);
                assert!(
                    (analytic - numeric).abs() / scale < 5e-3,
                    "{name} en ({plate}, {screen}, {grid}): {analytic} vs {numeric}"
                );
            }
        }
    }

    #[test]
    fn cutoff_regions_report_zero_slopes() {
        let model = model_6l6gc();
        let currents = model.currents_with_slopes(360.0, 1e-7, -18.0);
        assert_eq!(currents.plate_a, 0.0);
        assert_eq!(currents.d_plate_d_screen, 0.0);
        let currents = model.currents_with_slopes(360.0, 250.0, -200.0);
        assert_eq!(currents.screen_a, 0.0);
        assert_eq!(currents.d_screen_d_grid, 0.0);
    }

    #[test]
    fn plate_current_rises_with_screen_and_grid() {
        let model = model_6l6gc();
        let low = model.plate_current(400.0, 200.0, -20.0);
        let mid = model.plate_current(400.0, 250.0, -20.0);
        let high = model.plate_current(400.0, 250.0, -10.0);
        assert!(low < mid && mid < high, "{low} {mid} {high}");
    }

    #[test]
    fn knee_reduces_current_at_low_plate_voltage() {
        let model = model_6l6gc();
        let low_plate = model.plate_current(5.0, 250.0, -18.0);
        let high_plate = model.plate_current(360.0, 250.0, -18.0);
        assert!(low_plate > 0.0);
        assert!(low_plate < 0.35 * high_plate, "{low_plate} vs {high_plate}");
    }

    #[test]
    fn cutoff_and_guards_are_finite() {
        let model = model_6l6gc();
        // Corte suave (softplus): casi cero, no exactamente cero.
        assert!(model.plate_current(400.0, 250.0, -60.0) < 1e-4);
        assert!(model.plate_current(400.0, 250.0, -120.0) < 1e-9);
        assert_eq!(model.plate_current(400.0, 0.0, -18.0), 0.0);
        assert_eq!(model.screen_current(250.0, -60.0), 0.0);
        assert!(model.plate_current(1e6, 250.0, -18.0).is_finite());
        assert!(model.screen_current(1e6, 1e6).is_finite());
    }

    #[test]
    fn grid_conduction_is_soft_and_monotonic() {
        let model = model_6l6gc();
        let (below, slope_below) = model.grid_current_with_slope(-2.0);
        let (at, _) = model.grid_current_with_slope(0.0);
        let (above, slope_above) = model.grid_current_with_slope(1.0);
        assert!(below < 1e-9, "conducción residual en corte: {below}");
        assert!(slope_below < 1e-12, "pendiente residual: {slope_below}");
        assert!(at > 0.0 && above > at);
        assert!(slope_above > 0.0 && slope_above <= 1.0 / 400.0);
    }
}
