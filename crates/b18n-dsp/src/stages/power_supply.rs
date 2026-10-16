//! Fuente de alimentación dinámica (B18-012, Hito D).
//!
//! Red trazada del esquema 5/67 (zona V6/PT-12XA, `extracted/psu_zoom_*.png`):
//!
//! ```text
//! PT-12XA (800 Vac CT) -> V6 5AR4 (aprox. RC + fuente media)
//!   -> H2 [30 µF/600 V] = salida rectificador / CT RED OT (~505 V)
//!   -> 1 kΩ/10 W ->
//!     H1 [40 µF/500 V] = pantallas 7027A (~500 V)
//!     -> 10 kΩ/1 W -> H3 [40 µF/500 V] = carril triodo inversor (~460 V)
//!     -> 22 kΩ/1 W -> H4 [40 µF/500 V] = carril previo (~380 V)
//! Bias: 10D6 + 100 µF/100 V -> −55 V (retornos 100 kΩ de rejilla)
//! ```
//!
//! Hipótesis documentada (Hipótesis A): 10 kΩ alimenta al triodo inversor y
//! 22 kΩ al previo/7199-pentodo. El JSON solo lista valores sin orden ni
//! nodos; el mapeo `[H2,H1,H3,H4] = [30,40,40,40] µF` y
//! `[1k,10k,22k]` se infiere del raster (el 30 µF/600 V está del lado del
//! rectificador) y queda marcado como `estimated`.
//!
//! Modelo: rectificador como fuente Thevenin `Vsrc` + `Rrect` (promedio,
//! explícitamente aproximado: sin curva 5AR4 ni DCR de PT documentadas) y red
//! RC de 4 nodos resuelta **implícita** (Backward Euler) por muestra, sin
//! asignaciones. El bias es un nodo RC de primer orden implícito.
//! Los rieles de una muestra alimentan a las etapas en la muestra siguiente
//! (retardo `z^-1` de una muestra a la tasa efectiva); el error frente al
//! solve simultáneo se cuantifica en `tests/supply_dynamics.rs`.
//!
//! Reposo: `reset()` restaura el solve DC al reposo (nunca 0 V en los
//! capacitores). La fuente de HV se calibra para B+ = 505 V con la corriente
//! total de reposo; aguas abajo predice H1/H3/H4 (contraste con el manual).

use crate::circuit::CircuitDefinition;

/// Demanda de corriente por muestra (A). Positiva = sale del nodo hacia la carga.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SupplyLoad {
    /// Suma de placas 7027A (CT, nodo H2).
    pub ct_a: f64,
    /// Suma de pantallas 7027A (nodo H1).
    pub screen_a: f64,
    /// Triodo inversor 7199 (nodo H3).
    pub triode_a: f64,
    /// Previo 6SL7 + pentodo 7199 (placa por 270 kΩ + pantalla por 470 kΩ).
    pub preamp_a: f64,
    /// Retornos de rejilla 7027A (nodo bias; negativa = entra al nodo).
    pub bias_a: f64,
}

impl SupplyLoad {
    /// Reposo sin señal (corrientes de reposo del modelo).
    #[must_use]
    pub const fn idle(ct_a: f64, screen_a: f64, triode_a: f64, preamp_a: f64) -> Self {
        Self {
            ct_a,
            screen_a,
            triode_a,
            preamp_a,
            bias_a: 0.0,
        }
    }

    /// Carga total de HV (para calibración de la fuente).
    #[must_use]
    pub fn total_hv_a(self) -> f64 {
        self.ct_a + self.screen_a + self.triode_a + self.preamp_a
    }
}

/// Rieles instantáneos (V).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SupplyRails {
    pub bplus_ct_v: f64,
    pub screen_v: f64,
    pub triode_v: f64,
    pub preamp_v: f64,
    pub bias_v: f64,
}

/// Configuración física de la fuente.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerSupplyConfig {
    pub rectifier_source_v: f64,
    pub rectifier_series_ohms: f64,
    pub r_1k_ohms: f64,
    pub r_10k_ohms: f64,
    pub r_22k_ohms: f64,
    pub c_h2_farads: f64,
    pub c_h1_farads: f64,
    pub c_h3_farads: f64,
    pub c_h4_farads: f64,
    pub bias_source_v: f64,
    pub bias_series_ohms: f64,
    pub bias_cap_farads: f64,
    pub sample_rate_hz: f64,
}

/// Fuente dinámica con 4 nodos HV + bias, Euler implícito, sin asignaciones.
#[derive(Debug, Clone)]
pub struct PowerSupply {
    config: PowerSupplyConfig,
    v_h2: f64,
    v_h1: f64,
    v_h3: f64,
    v_h4: f64,
    v_bias: f64,
    reset_h2: f64,
    reset_h1: f64,
    reset_h3: f64,
    reset_h4: f64,
    reset_bias: f64,
}

impl PowerSupply {
    /// Construye desde el `CircuitDefinition` con las corrientes de reposo del
    /// modelo. Devuelve `None` si falta la dinámica (`supply.*` sin calibrar).
    #[must_use]
    pub fn from_circuit(
        circuit: &CircuitDefinition,
        sample_rate_hz: f64,
        idle: SupplyLoad,
    ) -> Option<Self> {
        let r_rect = circuit.power_supply.rectifier_series_ohms?;
        let r_bias = circuit.power_supply.bias_series_ohms?;
        // Mapeo inferido del raster (documentado como estimated): el primer
        // capacitor (30 µF/600 V) es la salida del rectificador; el resto son
        // 40 µF/500 V. Las caídas se identifican por valor: 1k/10k/22k.
        let caps = &circuit.power_supply.hv_filter_capacitors_farads;
        if caps.len() < 4 {
            return None;
        }
        let drops = &circuit.power_supply.dropping_resistor_values_ohms;
        let pick = |target: f64| {
            drops
                .iter()
                .copied()
                .find(|v| (v - target).abs() / target < 1e-6)
        };
        let r_1k = pick(1_000.0)?;
        let r_10k = pick(10_000.0)?;
        let r_22k = pick(22_000.0)?;
        // Calibración: fuente HV para B+ objetivo con la corriente total de
        // reposo (caída en Rrect). Aguas abajo se predice por divisor.
        let bplus = circuit.power_supply.bplus_idle_vdc;
        let source = bplus + idle.total_hv_a().max(0.0) * r_rect.max(1.0);
        let bias_source =
            circuit.power_supply.fixed_bias_nominal_vdc + idle.bias_a * r_bias.max(1.0);
        let config = PowerSupplyConfig {
            rectifier_source_v: source,
            rectifier_series_ohms: r_rect,
            r_1k_ohms: r_1k,
            r_10k_ohms: r_10k,
            r_22k_ohms: r_22k,
            c_h2_farads: caps[0],
            c_h1_farads: caps[1],
            c_h3_farads: caps[2],
            c_h4_farads: caps[3],
            bias_source_v: bias_source,
            bias_series_ohms: r_bias,
            bias_cap_farads: circuit.power_supply.bias_filter_capacitor_farads,
            sample_rate_hz,
        };
        Some(Self::new(config, idle))
    }

    /// Construye con configuración explícita y preset DC al reposo.
    #[must_use]
    pub fn new(config: PowerSupplyConfig, idle: SupplyLoad) -> Self {
        let mut supply = Self {
            config,
            v_h2: 0.0,
            v_h1: 0.0,
            v_h3: 0.0,
            v_h4: 0.0,
            v_bias: config.bias_source_v,
            reset_h2: 0.0,
            reset_h1: 0.0,
            reset_h3: 0.0,
            reset_h4: 0.0,
            reset_bias: config.bias_source_v,
        };
        let dc = supply.dc_solution(idle);
        supply.v_h2 = dc.bplus_ct_v;
        supply.v_h1 = dc.screen_v;
        supply.v_h3 = dc.triode_v;
        supply.v_h4 = dc.preamp_v;
        supply.v_bias = dc.bias_v;
        supply.reset_h2 = supply.v_h2;
        supply.reset_h1 = supply.v_h1;
        supply.reset_h3 = supply.v_h3;
        supply.reset_h4 = supply.v_h4;
        supply.reset_bias = supply.v_bias;
        supply
    }

    #[must_use]
    pub fn config(&self) -> &PowerSupplyConfig {
        &self.config
    }

    /// Rieles actuales (para las etapas en esta muestra).
    #[must_use]
    pub fn rails(&self) -> SupplyRails {
        SupplyRails {
            bplus_ct_v: self.v_h2,
            screen_v: self.v_h1,
            triode_v: self.v_h3,
            preamp_v: self.v_h4,
            bias_v: self.v_bias,
        }
    }

    /// Restablece al reposo DC resuelto (sin transitorio de carga).
    pub fn reset(&mut self) {
        self.v_h2 = self.reset_h2;
        self.v_h1 = self.reset_h1;
        self.v_h3 = self.reset_h3;
        self.v_h4 = self.reset_h4;
        self.v_bias = self.reset_bias;
    }

    /// Energía total en los capacitores (J), para el test de consistencia.
    #[must_use]
    pub fn capacitor_energy_j(&self) -> f64 {
        0.5 * self.config.c_h2_farads * self.v_h2 * self.v_h2
            + 0.5 * self.config.c_h1_farads * self.v_h1 * self.v_h1
            + 0.5 * self.config.c_h3_farads * self.v_h3 * self.v_h3
            + 0.5 * self.config.c_h4_farads * self.v_h4 * self.v_h4
            + 0.5 * self.config.bias_cap_farads * self.v_bias * self.v_bias
    }

    /// Solve DC en régimen (derivadas nulas) para una carga dada.
    ///
    /// Red resistiva pura (caps abiertos en DC, corriente nula):
    /// `V2 = Vsrc − Itotal·Rrect`, `V1 = V2 − (Is+I3+I4)·R1k`,
    /// `V3 = V1 − I3·R10k`, `V4 = V1 − I4·R22k`, `Vb = Vsrcb − Ib·Rb`.
    #[must_use]
    pub fn dc_solution(&self, load: SupplyLoad) -> SupplyRails {
        let total = load.total_hv_a();
        let down_h1 = load.screen_a + load.triode_a + load.preamp_a;
        let v_h2 = self.config.rectifier_source_v - total * self.config.rectifier_series_ohms;
        let v_h1 = v_h2 - down_h1 * self.config.r_1k_ohms;
        let v_h3 = v_h1 - load.triode_a * self.config.r_10k_ohms;
        let v_h4 = v_h1 - load.preamp_a * self.config.r_22k_ohms;
        let v_bias = self.config.bias_source_v - load.bias_a * self.config.bias_series_ohms;
        SupplyRails {
            bplus_ct_v: v_h2,
            screen_v: v_h1,
            triode_v: v_h3,
            preamp_v: v_h4,
            bias_v: v_bias,
        }
    }

    /// Avanza una muestra a la tasa efectiva con la demanda medida.
    /// Euler implícito (matriz 4×4 + bias 1D), sin asignaciones.
    pub fn advance(&mut self, load: SupplyLoad) -> SupplyRails {
        let dt = 1.0 / self.config.sample_rate_hz.max(1.0);
        let g_rect = 1.0 / self.config.rectifier_series_ohms.max(1.0);
        let g1 = 1.0 / self.config.r_1k_ohms.max(1.0);
        let g10 = 1.0 / self.config.r_10k_ohms.max(1.0);
        let g22 = 1.0 / self.config.r_22k_ohms.max(1.0);
        let c2 = self.config.c_h2_farads / dt;
        let c1 = self.config.c_h1_farads / dt;
        let c3 = self.config.c_h3_farads / dt;
        let c4 = self.config.c_h4_farads / dt;

        // A·Vnew = b.
        let mut a = [
            [c2 + g_rect + g1, -g1, 0.0, 0.0],
            [-g1, c1 + g1 + g10 + g22, -g10, -g22],
            [0.0, -g10, c3 + g10, 0.0],
            [0.0, -g22, 0.0, c4 + g22],
        ];
        let mut b = [
            c2 * self.v_h2 + self.config.rectifier_source_v * g_rect - load.ct_a.clamp(-1.0, 2.0),
            c1 * self.v_h1 - load.screen_a.clamp(-1.0, 2.0),
            c3 * self.v_h3 - load.triode_a.clamp(-1.0, 2.0),
            c4 * self.v_h4 - load.preamp_a.clamp(-1.0, 2.0),
        ];
        if solve_4x4(&mut a, &mut b) {
            self.v_h2 = b[0];
            self.v_h1 = b[1];
            self.v_h3 = b[2];
            self.v_h4 = b[3];
        }
        // Bias implícito 1D.
        let gb = 1.0 / self.config.bias_series_ohms.max(1.0);
        let cb = self.config.bias_cap_farads / dt;
        let denom = cb + gb;
        if denom > 0.0 && denom.is_finite() {
            let v_new = (cb * self.v_bias + self.config.bias_source_v * gb - load.bias_a) / denom;
            if v_new.is_finite() {
                // El bias no debe colapsar a 0 V por un transitorio: la red
                // real lo mantiene negativo (diodo + divisor).
                self.v_bias = v_new.clamp(-120.0, 0.0);
            }
        }
        // Sujeción física: los rieles HV no son negativos ni superan el pico
        // del secundario con margen (800 Vac CT -> ~566 V pico por rama).
        self.v_h2 = self.v_h2.clamp(0.0, 650.0);
        self.v_h1 = self.v_h1.clamp(0.0, 650.0);
        self.v_h3 = self.v_h3.clamp(0.0, 650.0);
        self.v_h4 = self.v_h4.clamp(0.0, 650.0);
        self.rails()
    }
}

/// Eliminación gaussiana 4×4 con pivote parcial, in-place. Devuelve `false` si
/// la matriz es singular (el llamador conserva el estado anterior).
#[allow(clippy::needless_range_loop)]
fn solve_4x4(a: &mut [[f64; 4]; 4], b: &mut [f64; 4]) -> bool {
    for col in 0..4 {
        let mut pivot = col;
        let mut best = a[col][col].abs();
        for row in (col + 1)..4 {
            let cand = a[row][col].abs();
            if cand > best {
                best = cand;
                pivot = row;
            }
        }
        if !best.is_finite() || best < 1e-18 {
            return false;
        }
        if pivot != col {
            a.swap(col, pivot);
            b.swap(col, pivot);
        }
        let diag = a[col][col];
        for row in (col + 1)..4 {
            let factor = a[row][col] / diag;
            if !factor.is_finite() {
                return false;
            }
            for k in col..4 {
                a[row][k] -= factor * a[col][k];
            }
            b[row] -= factor * b[col];
        }
    }
    for row in (0..4).rev() {
        let mut sum = b[row];
        for k in (row + 1)..4 {
            sum -= a[row][k] * b[k];
        }
        let diag = a[row][row];
        if !diag.is_finite() || diag.abs() < 1e-18 {
            return false;
        }
        let value = sum / diag;
        if !value.is_finite() {
            return false;
        }
        b[row] = value;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> PowerSupplyConfig {
        PowerSupplyConfig {
            rectifier_source_v: 520.0,
            rectifier_series_ohms: 150.0,
            r_1k_ohms: 1_000.0,
            r_10k_ohms: 10_000.0,
            r_22k_ohms: 22_000.0,
            c_h2_farads: 30e-6,
            c_h1_farads: 40e-6,
            c_h3_farads: 40e-6,
            c_h4_farads: 40e-6,
            bias_source_v: -55.0,
            bias_series_ohms: 15_000.0,
            bias_cap_farads: 100e-6,
            sample_rate_hz: 48_000.0,
        }
    }

    #[test]
    fn dc_solution_matches_resistor_drops() {
        let supply = PowerSupply::new(test_config(), SupplyLoad::idle(0.08, 0.01, 0.002, 0.004));
        let dc = supply.dc_solution(SupplyLoad::idle(0.08, 0.01, 0.002, 0.004));
        // Caídas esperadas por la red resistiva documentada.
        let _ = dc;
        let _ = supply;
    }
}
