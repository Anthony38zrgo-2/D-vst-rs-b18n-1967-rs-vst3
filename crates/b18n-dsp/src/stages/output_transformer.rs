//! Transformador de salida OT-471 (B18-013).
//!
//! Dos topologías, seleccionadas por los parámetros presentes:
//!
//! - [`OutputTransformerTopology::Ideal`] — regresión/fallback del Hito C:
//!   conversión estática `vsec = (Vp1−Vp2)/N · Rc/(Rc+Rdc_sec)` con la
//!   impedancia nominal placa-placa en la ecuación diferencial y el paso alto
//!   de 1 Hz como bloqueo de continua declarado.
//! - [`OutputTransformerTopology::Linear`] — red diferencial con parásitos
//!   estimados (`estimated`, véase `docs/MODEL_ASSUMPTIONS.md`):
//!
//!   ```text
//!   Vp1−Vp2 = −[ 2·Rh + R_core + (Lm ∥ (Lleak + N²·(Rcarga+Rdc_sec))) ∥ Cp ]·id
//!   ```
//!
//!   con `id = (ip1 − ip2)/2` la corriente diferencial. El secundario se
//!   acopla a la **tensión del nodo interno F** (el EMF del camino de flujo,
//!   `Lm ∥ rama reflejada`), de modo que la componente continua no se
//!   transfiere. La caída se discretiza con TPT (condensador) y Euler atrás
//!   (inductores) y sustituye el paso alto artificial. La resistencia serie
//!   `R_core` representa las pérdidas del núcleo: acota la deriva del
//!   integrador de flujo (una inductancia pura no tiene fuerza restauradora de
//!   continua) mientras el núcleo no disponga de modelo magnético no lineal
//!   (B18-014); su valor está etiquetado `estimated` y su efecto en la banda
//!   audible es < 0,1 dB en la esquina.
//!
//! Convención de signos (idéntica al solve de placas del Hito C): `id > 0`
//! (placa 1 conduce más) baja la placa 1 y produce secundario negativo,
//! `vsec = −N·i_reflejada·Zcarga`.
//!
//! **B18-015:** la carga del secundario ([`ElectricalLoadConfig`]) entra en la
//! rama reflejada del solve como puerto Norton `i = G·v + Ieq` (resistencia
//! nominal o equivalente RLC estimado del altavoz); la tensión de terminal y
//! la corriente de secundario salen del mismo avance. El modo ideal de
//! regresión usa `load.nominal_ohms()` en su conversión estática.

use crate::numerics::discretization::{InductorIntegration, TptCapacitor, TptInductor};
use crate::stages::speaker_impedance::{ElectricalLoad, ElectricalLoadConfig};
use crate::utilities::filters::OnePoleHighPass;

/// Corte del bloqueo de continua del **modo ideal** (fallback del Hito C; el
/// modo lineal no lo necesita: `Lm`/`Lleak` bloquean la continua física).
const IDEAL_DC_BLOCK_CUTOFF_HZ: f64 = 1.0;

/// Parámetros físicos del transformador de salida.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputTransformerConfig {
    /// Impedancia nominal placa-placa del secundario ideal (4300 Ω, modo ideal).
    pub primary_plate_to_plate_ohms_nominal: f64,
    /// Relación de transformación primario (total) a secundario de 8 Ω.
    pub turns_ratio_8ohm: f64,
    /// Relación de transformación primario (total) a la toma de 16 Ω.
    pub turns_ratio_16ohm: f64,
    /// DCR del primario completo, placa a placa (modo lineal: 2·Rh en serie).
    pub dc_primary_winding_ohms: f64,
    pub dc_secondary_8ohm_winding_ohms: f64,
    /// Carga eléctrica del secundario (B18-015): resistencia nominal del JSON
    /// o equivalente RLC estimado del altavoz. Entra en el solve.
    pub load: ElectricalLoadConfig,
    /// Inductancia de magnetización diferencial (placa-placa), `None` = sin
    /// caracterizar (modo ideal).
    pub magnetizing_inductance_h: Option<f64>,
    /// Inductancia de fuga diferencial, referida al primario.
    pub leakage_inductance_h: Option<f64>,
    /// Capacidad parásita equivalente placa-placa.
    pub parasitic_capacitance_f: Option<f64>,
    /// Resistencia serie del camino de flujo (pérdidas de núcleo, `estimated`).
    pub core_loss_ohms: Option<f64>,
    /// Cota de la corriente de magnetización (`estimated`): limitador
    /// numérico provisional del flujo mientras el núcleo no tenga modelo
    /// magnético no lineal (B18-014). No es una curva de saturación.
    pub magnetizing_current_limit_a: Option<f64>,
}

/// Topología activa del modelo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputTransformerTopology {
    /// Conversión estática del Hito C (regresión y fallback explícito).
    Ideal,
    /// Red diferencial con Lm/Lleak/Cp estimados (B18-013).
    Linear,
}

/// Configuración inválida del transformador o de su carga.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputTransformerError {
    IncompleteParasiticModel,
    /// Parámetros de carga degenerados (no finitos o no positivos).
    InvalidLoadModel,
}

impl std::fmt::Display for OutputTransformerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IncompleteParasiticModel => write!(
                formatter,
                "el modelo del OT-471 está incompleto: Lm, Lleak y Cp deben estar los tres \
                 presentes (modo lineal) o los tres ausentes (modo ideal)"
            ),
            Self::InvalidLoadModel => write!(
                formatter,
                "la carga eléctrica del secundario no es válida (parámetros no positivos o no \
                 finitos)"
            ),
        }
    }
}

impl std::error::Error for OutputTransformerError {}

/// Transformador de salida OT-471 con estado temporal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputTransformerModel {
    config: OutputTransformerConfig,
    topology: OutputTransformerTopology,
    dt_seconds: f64,
    /// DCR total en serie por el lazo diferencial (2 medias bobinas).
    primary_series_ohms: f64,
    /// Red de carga del secundario (puerto Norton en los terminales).
    load: ElectricalLoad,
    /// Rama reflejada `N8²·(Z_carga + DCR_sec)` vista desde el primario
    /// (constante para un `dt` y configuración dados).
    reflected_conductance: f64,
    /// `G_leak/(G_leak+G_ref)`: peso de la fuente Norton reflejada.
    reflected_current_gain: f64,
    /// `1/(1 + DCR_sec·G_carga)`: DCR de secundario en serie con la carga.
    secondary_source_gain: f64,
    leakage_conductance: f64,
    /// `G_ref/(G_leak+G_ref)`: fracción de corriente de la rama de fuga.
    leakage_shunt_ratio: f64,
    /// `1/(1 + R_core·G_F)`: reparto de la KCL del nodo interno F.
    flux_series_ratio: f64,
    parallel_conductance: f64,
    magnetizing: TptInductor,
    leakage: TptInductor,
    parasitic: TptCapacitor,
    dc_block: OnePoleHighPass,
    secondary_v: f64,
    secondary_current_a: f64,
    magnetizing_current_a: f64,
    reflected_current_a: f64,
    /// El limitador de flujo actuó en el último avance (diagnóstico B18-013).
    flux_limited: bool,
}

/// Constantes de la red lineal derivadas de la carga (B18-015): dependen de
/// `dt` y de la configuración, no del estado por muestra.
#[derive(Debug, Clone, Copy)]
struct LinearConstants {
    reflected_conductance: f64,
    reflected_current_gain: f64,
    secondary_source_gain: f64,
    leakage_shunt_ratio: f64,
    flux_series_ratio: f64,
    parallel_conductance: f64,
}

/// Resultado del avance de un paso (salidas registradas por el modelo).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputTransformerOutputs {
    pub secondary_v: f64,
    /// Corriente de secundario del último avance (positiva hacia la carga).
    pub secondary_current_a: f64,
    /// Corriente por la rama de magnetización (flujo diferencial).
    pub magnetizing_current_a: f64,
    /// Corriente por la rama reflejada (fuga + carga).
    pub reflected_current_a: f64,
}

impl OutputTransformerModel {
    /// Crea el modelo para un paso `dt_seconds`. La topología se elige por los
    /// parásitos presentes: los cuatro `Some` → lineal; los cuatro `None` →
    /// ideal; combinación parcial → error explícito (no se degrada en
    /// silencio). Una carga degenerada también es error explícito.
    pub fn new(
        config: OutputTransformerConfig,
        dt_seconds: f64,
    ) -> Result<Self, OutputTransformerError> {
        let topology = match (
            config.magnetizing_inductance_h,
            config.leakage_inductance_h,
            config.parasitic_capacitance_f,
            config.core_loss_ohms,
            config.magnetizing_current_limit_a,
        ) {
            (Some(_), Some(_), Some(_), Some(_), Some(_)) => OutputTransformerTopology::Linear,
            (None, None, None, None, None) => OutputTransformerTopology::Ideal,
            _ => return Err(OutputTransformerError::IncompleteParasiticModel),
        };
        if !config.load.is_physically_valid() {
            return Err(OutputTransformerError::InvalidLoadModel);
        }

        let primary_series_ohms = config.dc_primary_winding_ohms;
        // Carga del secundario (B18-015): puerto Norton `i = G·v + Ieq` en los
        // terminales del altavoz, dentro del mismo solve que la red del OT.
        let load = ElectricalLoad::new(config.load, dt_seconds);

        // En el modo ideal los elementos reactivos quedan inertes (estados a
        // cero, nunca se avanzan); se construyen con valores unitarios.
        // El camino de flujo usa Euler atrás (disipativo): la memoria del
        // integrador de flujo es solo de corriente y queda acotada por el
        // limitador provisional (B18-014).
        let magnetizing = TptInductor::with_integration(
            config.magnetizing_inductance_h.unwrap_or(1.0),
            dt_seconds,
            InductorIntegration::BackwardEuler,
        );
        // Tanto la fuga como el flujo usan Euler atrás: la memoria queda
        // acotada (robustez del solve ante transitorios violentos) a cambio de
        // una desviación declarada en la esquina de agudos (≫ banda audible;
        // cuantificada en el contraste con SPICE, `output_transformer_ac.rs`).
        let leakage = TptInductor::with_integration(
            config.leakage_inductance_h.unwrap_or(1.0),
            dt_seconds,
            InductorIntegration::BackwardEuler,
        );
        let parasitic =
            TptCapacitor::new(config.parasitic_capacitance_f.unwrap_or(1e-12), dt_seconds);

        let constants = Self::linear_constants(&config, &load, &magnetizing, &leakage, &parasitic);

        Ok(Self {
            config,
            topology,
            dt_seconds,
            primary_series_ohms,
            load,
            reflected_conductance: constants.reflected_conductance,
            reflected_current_gain: constants.reflected_current_gain,
            secondary_source_gain: constants.secondary_source_gain,
            leakage_conductance: leakage.conductance(),
            leakage_shunt_ratio: constants.leakage_shunt_ratio,
            flux_series_ratio: constants.flux_series_ratio,
            parallel_conductance: constants.parallel_conductance,
            magnetizing,
            leakage,
            parasitic,
            dc_block: OnePoleHighPass::new(IDEAL_DC_BLOCK_CUTOFF_HZ, 1.0 / dt_seconds),
            secondary_v: 0.0,
            secondary_current_a: 0.0,
            magnetizing_current_a: 0.0,
            reflected_current_a: 0.0,
            flux_limited: false,
        })
    }

    /// Deriva las constantes de la red lineal de la carga actual: la rama
    /// reflejada `N8²·(Z_carga + DCR_sec)` en conductancia, el reparto de
    /// fuga/reflejo y la KCL del nodo interno F.
    fn linear_constants(
        config: &OutputTransformerConfig,
        load: &ElectricalLoad,
        magnetizing: &TptInductor,
        leakage: &TptInductor,
        parasitic: &TptCapacitor,
    ) -> LinearConstants {
        // DCR de secundario en serie con la carga: Norton equivalente.
        let secondary_source_gain =
            1.0 / (1.0 + config.dc_secondary_8ohm_winding_ohms * load.conductance());
        let reflected_conductance = load.conductance() * secondary_source_gain
            / (config.turns_ratio_8ohm * config.turns_ratio_8ohm);
        let leakage_conductance = leakage.conductance();
        let series_denominator = leakage_conductance + reflected_conductance;
        let leakage_shunt_ratio = reflected_conductance / series_denominator;
        let reflected_current_gain = leakage_conductance / series_denominator;
        // Nodo interno F: `Lm ∥ rama reflejada`; la DCR primaria llega por
        // `R_core` (pérdidas de flujo) en serie desde el nodo de placas.
        let core_loss_ohms = config.core_loss_ohms.unwrap_or(0.0);
        let node_f_conductance =
            magnetizing.conductance() + leakage_conductance * leakage_shunt_ratio;
        let flux_series_ratio = 1.0 / (1.0 + core_loss_ohms * node_f_conductance);
        let parallel_conductance = node_f_conductance * flux_series_ratio + parasitic.conductance();
        LinearConstants {
            reflected_conductance,
            reflected_current_gain,
            secondary_source_gain,
            leakage_shunt_ratio,
            flux_series_ratio,
            parallel_conductance,
        }
    }

    /// Cambia la carga del secundario (B18-015) sin reiniciar el estado del
    /// transformador. La red de carga arranca descargada (transitorio breve,
    /// declarado: es una selección de configuración, no un mando de ejecución).
    ///
    /// Precondición: `load.is_physically_valid()` (validado al construir el
    /// motor; véase [`Self::new`]).
    pub fn set_load(&mut self, load: ElectricalLoadConfig) {
        debug_assert!(load.is_physically_valid());
        if self.config.load == load {
            return;
        }
        self.config.load = load;
        self.load = ElectricalLoad::new(load, self.dt_seconds);
        let constants = Self::linear_constants(
            &self.config,
            &self.load,
            &self.magnetizing,
            &self.leakage,
            &self.parasitic,
        );
        self.reflected_conductance = constants.reflected_conductance;
        self.reflected_current_gain = constants.reflected_current_gain;
        self.secondary_source_gain = constants.secondary_source_gain;
        self.leakage_shunt_ratio = constants.leakage_shunt_ratio;
        self.flux_series_ratio = constants.flux_series_ratio;
        self.parallel_conductance = constants.parallel_conductance;
    }

    #[must_use]
    pub fn topology(&self) -> OutputTransformerTopology {
        self.topology
    }

    #[must_use]
    pub fn config(&self) -> &OutputTransformerConfig {
        &self.config
    }

    /// Tensión de secundario (terminales del altavoz) del último avance.
    #[must_use]
    pub fn secondary_v(&self) -> f64 {
        self.secondary_v
    }

    /// Corriente de secundario del último avance (positiva hacia la carga).
    #[must_use]
    pub fn secondary_current_a(&self) -> f64 {
        self.secondary_current_a
    }

    /// Carga activa del secundario (B18-015; diagnóstico y selección).
    #[must_use]
    pub fn load_config(&self) -> ElectricalLoadConfig {
        self.config.load
    }

    /// Corriente de magnetización diferencial (diagnóstico de flujo/offset).
    #[must_use]
    pub fn magnetizing_current_a(&self) -> f64 {
        self.magnetizing_current_a
    }

    /// Corriente por la rama reflejada (fuga + carga).
    #[must_use]
    pub fn reflected_current_a(&self) -> f64 {
        self.reflected_current_a
    }

    /// El limitador provisional de flujo actuó en el último avance.
    #[must_use]
    pub fn flux_limited(&self) -> bool {
        self.flux_limited
    }

    /// Estado inicial congruente: sin excitación diferencial no hay flujo,
    /// corriente ni desplazamiento de continua.
    pub fn reset(&mut self) {
        self.magnetizing.reset();
        self.leakage.reset();
        self.parasitic.reset();
        self.dc_block.reset();
        self.load.reset();
        self.secondary_v = 0.0;
        self.secondary_current_a = 0.0;
        self.magnetizing_current_a = 0.0;
        self.reflected_current_a = 0.0;
    }

    /// Caída diferencial `drop = −(Vp1−Vp2)` y su pendiente con los estados
    /// actuales (afín en `id`; no muta el estado). Incluye la DCR primaria.
    #[must_use]
    pub fn differential_drop(&self, differential_current_a: f64) -> (f64, f64) {
        let slope = match self.topology {
            OutputTransformerTopology::Ideal => {
                self.primary_series_ohms + self.config.primary_plate_to_plate_ohms_nominal
            }
            OutputTransformerTopology::Linear => {
                self.primary_series_ohms + 1.0 / self.parallel_conductance
            }
        };
        (slope * differential_current_a + self.history_drop(), slope)
    }

    /// Fuente Norton de la rama reflejada vista desde el primario:
    /// `i = G_ref·v + I_ref` con `I_ref = −I_carga·(1/(1+DCR·G_carga))/N8`
    /// (reflejo de corriente secundario→primario con la convención del motor).
    fn reflected_source_current_a(&self) -> f64 {
        -self.load.companion_current() * self.secondary_source_gain / self.config.turns_ratio_8ohm
    }

    /// Fuente Norton de la rama serie `Lleak + carga reflejada` vista desde el
    /// nodo interno F: `I_s = I_leak·shunt + I_ref·(G_leak/(G_leak+G_ref))`.
    fn leakage_series_source_a(&self) -> f64 {
        self.leakage.companion_current() * self.leakage_shunt_ratio
            + self.reflected_source_current_a() * self.reflected_current_gain
    }

    /// Término independiente de la caída (historia de los elementos TPT).
    fn history_drop(&self) -> f64 {
        match self.topology {
            OutputTransformerTopology::Ideal => 0.0,
            OutputTransformerTopology::Linear => {
                // KCL en F: `i_R = i_m + i_ref`; el condensador cuelga del nodo
                // de placas y la DCR de pérdidas va en serie con F.
                let node_f_history =
                    self.magnetizing.companion_current() + self.leakage_series_source_a();
                let history_b =
                    node_f_history * self.flux_series_ratio - self.parasitic.companion_current();
                -history_b / self.parallel_conductance
            }
        }
    }

    /// Corriente diferencial demandada por una tensión diferencial impuesta
    /// (red de 1 puerto lineal; independiente del solve de placas).
    #[must_use]
    pub fn differential_current(&self, plate_difference_v: f64) -> f64 {
        let (drop_at_zero, slope) = self.differential_drop(0.0);
        -(plate_difference_v + drop_at_zero) / slope
    }

    /// Read-only affine secondary port `(offset, dVsecondary/dVprimary)`.
    /// Used by the simultaneous cathode-feedback solve. No capacitor,
    /// inductor or load history is advanced while evaluating Newton trials.
    #[must_use]
    pub fn secondary_voltage_port(&self) -> (f64, f64) {
        if self.topology == OutputTransformerTopology::Ideal {
            let mut zero = *self;
            let mut one = *self;
            zero.advance(0.0, self.differential_current(0.0));
            one.advance(1.0, self.differential_current(1.0));
            return (zero.secondary_v(), one.secondary_v() - zero.secondary_v());
        }
        let (history, slope) = self.differential_drop(0.0);
        let parallel_zero = -history / (slope * self.parallel_conductance) + history;
        let series_source = self.leakage_series_source_a();
        let history_f = self.magnetizing.companion_current() + series_source;
        let core = self.config.core_loss_ohms.unwrap_or(0.0);
        let flux_zero = (parallel_zero - core * history_f) * self.flux_series_ratio;
        let flux_gain = -self.flux_series_ratio / (slope * self.parallel_conductance);
        let node_g_zero = flux_zero * (1.0 - self.leakage_shunt_ratio)
            - (series_source - self.leakage.companion_current()) / self.leakage_conductance;
        let node_g_gain = flux_gain * (1.0 - self.leakage_shunt_ratio);
        let n = self.config.turns_ratio_8ohm;
        let offset = (-node_g_zero / n
            - self.config.dc_secondary_8ohm_winding_ohms * self.load.companion_current())
            * self.secondary_source_gain;
        (offset, -node_g_gain / n * self.secondary_source_gain)
    }

    /// Avanza el estado con la solución `(Vp1−Vp2, id)` y devuelve las
    /// salidas. En modo ideal replica la conversión estática del Hito C.
    pub fn advance(
        &mut self,
        plate_difference_v: f64,
        differential_current_a: f64,
    ) -> OutputTransformerOutputs {
        match self.topology {
            OutputTransformerTopology::Ideal => {
                let load_ohms = self.config.load.nominal_ohms();
                let secondary = plate_difference_v / self.config.turns_ratio_8ohm * load_ohms
                    / (load_ohms + self.config.dc_secondary_8ohm_winding_ohms);
                self.secondary_v = self.dc_block.process(secondary);
                self.secondary_current_a = self.load.advance(self.secondary_v);
            }
            OutputTransformerTopology::Linear => {
                let parallel_v =
                    differential_current_a / self.parallel_conductance + self.history_drop();
                let core_loss_ohms = self.config.core_loss_ohms.unwrap_or(0.0);
                // Nodo interno F: `v_F = (uA − R_core·hist_F)/(1 + R_core·G_F)`.
                let series_source = self.leakage_series_source_a();
                let node_f_history = self.magnetizing.companion_current() + series_source;
                let flux_v =
                    (parallel_v - core_loss_ohms * node_f_history) * self.flux_series_ratio;
                // Rama serie Lleak + carga reflejada: `i_ref = G_s·v_F + I_s`.
                let leakage_companion = self.leakage.companion_current();
                let reflected =
                    self.leakage_conductance * flux_v * self.leakage_shunt_ratio + series_source;
                // Tensión tras Lleak (nodo G): `v_G = v_F − (i_ref − I_leak)/G_leak`.
                let node_g_v = flux_v - (reflected - leakage_companion) / self.leakage_conductance;
                self.leakage.update(flux_v - node_g_v);
                self.magnetizing.update(flux_v);
                // Limitador provisional del flujo (placeholder de saturación,
                // B18-014): acota la memoria del integrador para que ningún
                // transitorio patológico deje una corriente de flujo ilimitada.
                self.flux_limited = false;
                if let Some(limit) = self.config.magnetizing_current_limit_a {
                    let current = self.magnetizing.current();
                    if current.abs() > limit {
                        self.magnetizing.preset_current(limit.copysign(current));
                        self.flux_limited = true;
                    }
                }
                self.parasitic.update(parallel_v);
                self.reflected_current_a = reflected;
                self.magnetizing_current_a = self.magnetizing.current();
                // Secundario acoplado al EMF del flujo (bloqueo de continua
                // físico): `e = −v_G/N8`; la DCR de secundario va en serie con
                // la carga (Norton de la carga resuelto en el mismo paso).
                let emf_secondary_v = -node_g_v / self.config.turns_ratio_8ohm;
                let load_companion = self.load.companion_current();
                let secondary_current =
                    self.load.conductance() * self.secondary_source_gain * emf_secondary_v
                        + load_companion * self.secondary_source_gain;
                let load_v = (secondary_current - load_companion) / self.load.conductance();
                self.load.advance(load_v);
                self.secondary_current_a = secondary_current;
                self.secondary_v = load_v;
            }
        }
        OutputTransformerOutputs {
            secondary_v: self.secondary_v,
            secondary_current_a: self.secondary_current_a,
            magnetizing_current_a: self.magnetizing_current_a,
            reflected_current_a: self.reflected_current_a,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear_config() -> OutputTransformerConfig {
        OutputTransformerConfig {
            primary_plate_to_plate_ohms_nominal: 4_300.0,
            turns_ratio_8ohm: 23.18,
            turns_ratio_16ohm: 16.39,
            dc_primary_winding_ohms: 300.0,
            dc_secondary_8ohm_winding_ohms: 0.4,
            load: ElectricalLoadConfig::Resistive { ohms: 8.0 },
            magnetizing_inductance_h: Some(28.7),
            leakage_inductance_h: Some(0.012),
            parasitic_capacitance_f: Some(100e-12),
            core_loss_ohms: Some(500.0),
            magnetizing_current_limit_a: Some(0.12),
        }
    }

    fn ideal_config() -> OutputTransformerConfig {
        OutputTransformerConfig {
            magnetizing_inductance_h: None,
            leakage_inductance_h: None,
            parasitic_capacitance_f: None,
            core_loss_ohms: None,
            magnetizing_current_limit_a: None,
            ..linear_config()
        }
    }

    #[test]
    fn ideal_topology_matches_static_conversion() {
        let mut model =
            OutputTransformerModel::new(ideal_config(), 1.0 / 48_000.0).expect("modelo");
        assert_eq!(model.topology(), OutputTransformerTopology::Ideal);
        let outputs = model.advance(1.0, 0.0);
        let expected = 1.0 / 23.18 * 8.0 / 8.4;
        // Primer sample: el paso alto de 1 Hz apenas atenúa.
        assert!(
            (outputs.secondary_v - expected).abs() < expected * 1e-3,
            "{} vs {expected}",
            outputs.secondary_v
        );
        // Con excitación constante el paso alto (fallback) drena la continua.
        for _ in 0..480_000 {
            model.advance(1.0, 0.0);
        }
        assert!(model.secondary_v().abs() < 1e-3);
    }

    #[test]
    fn incomplete_parasitic_model_is_rejected() {
        let mut config = linear_config();
        config.leakage_inductance_h = None;
        assert_eq!(
            OutputTransformerModel::new(config, 1.0 / 48_000.0),
            Err(OutputTransformerError::IncompleteParasiticModel)
        );
    }

    #[test]
    fn linear_dc_is_blocked_and_flows_through_magnetizing_branch() {
        let dt = 1.0 / 192_000.0;
        let mut model = OutputTransformerModel::new(linear_config(), dt).expect("modelo");
        let vd = 2.0;
        for _ in 0..400_000 {
            let id = model.differential_current(vd);
            model.advance(vd, id);
        }
        // La continua diferencial no llega al secundario.
        assert!(model.secondary_v().abs() < 1e-3, "{}", model.secondary_v());
        // Circula por la rama de magnetización (no por la carga).
        assert!(model.magnetizing_current_a().abs() > 1e-3);
        assert!(model.reflected_current_a().abs() < 1e-6);
        // En régimen estacionario la caída óhmica es la de la DCR primaria más
        // la resistencia de pérdidas del camino de flujo.
        let expected_id = -vd / 800.0;
        assert!(
            (model.magnetizing_current_a() - expected_id).abs() < 1e-3,
            "{} vs {expected_id}",
            model.magnetizing_current_a()
        );
    }

    #[test]
    fn midband_transimpedance_matches_reflected_load() {
        let rate = 192_000.0_f64;
        let dt = 1.0 / rate;
        let config = linear_config();
        let mut model = OutputTransformerModel::new(config, dt).expect("modelo");
        let frequency = 1_000.0_f64;
        let omega = 2.0 * std::f64::consts::PI * frequency;
        let frames = 96_000;
        let (mut re, mut im) = (0.0, 0.0);
        for index in 0..frames {
            let id = (omega * index as f64 * dt).sin();
            let (drop, _) = model.differential_drop(id);
            let outputs = model.advance(-drop, id);
            if index >= frames / 2 {
                let angle = omega * index as f64 * dt;
                re += outputs.secondary_v * angle.cos();
                im -= outputs.secondary_v * angle.sin();
            }
        }
        // Excitación de corriente (pentodos ≈ fuente de corriente en
        // pequeño señal): `vsec/id = −N·Rcarga` en la banda media.
        let magnitude = re.hypot(im) / (frames as f64 / 4.0);
        let expected = 23.18 * 8.0;
        let error = (magnitude - expected).abs() / expected;
        assert!(error < 0.02, "transimpedancia {magnitude} vs {expected}");
    }

    #[test]
    fn low_frequency_corner_is_set_by_magnetizing_inductance() {
        let rate = 192_000.0_f64;
        let dt = 1.0 / rate;
        let config = linear_config();
        let corner_hz = config.turns_ratio_8ohm * config.turns_ratio_8ohm * 8.4
            / (2.0 * std::f64::consts::PI * 28.7);
        assert!((corner_hz - 25.0).abs() < 0.5, "esquina {corner_hz}");

        // Amplitud pequeña: la corriente de flujo queda por debajo de la cota
        // provisional (0,12 A) y el limitador no interviene.
        let amplitude = 0.05_f64;
        let gain_at = |frequency: f64| -> f64 {
            let mut model = OutputTransformerModel::new(config, dt).expect("modelo");
            let omega = 2.0 * std::f64::consts::PI * frequency;
            let frames = (8.0 * rate / frequency) as usize;
            let (mut re, mut im) = (0.0, 0.0);
            for index in 0..frames {
                let id = amplitude * (omega * index as f64 * dt).sin();
                let (drop, _) = model.differential_drop(id);
                let outputs = model.advance(-drop, id);
                assert!(!model.flux_limited(), "limitador de flujo activo");
                if index >= frames / 2 {
                    let angle = omega * index as f64 * dt;
                    re += outputs.secondary_v * angle.cos();
                    im -= outputs.secondary_v * angle.sin();
                }
            }
            re.hypot(im) / (frames as f64 / 4.0)
        };
        let mid = gain_at(200.0);
        let at_corner = gain_at(corner_hz);
        let ratio_db = 20.0 * (at_corner / mid).log10();
        assert!(
            (ratio_db + 3.0103).abs() < 0.35,
            "esquina LF {ratio_db:.2} dB"
        );
    }

    #[test]
    fn high_frequency_response_is_set_by_leakage_and_capacitance() {
        let rate = 768_000.0_f64;
        let dt = 1.0 / rate;
        let config = linear_config();

        // Excitación de tensión pura (Rs = 0): la esquina de la fuga sola
        // queda en f = (2·Rh + R_reflejada)/(2π·Lleak) ≈ 64 kHz. Se anula `Cp`
        // para aislar la fuga (con 100 pF la capacidad añade ≈1 dB en la
        // esquina, validado aparte contra SPICE).
        let leakage_only = OutputTransformerConfig {
            parasitic_capacitance_f: Some(1e-15),
            ..linear_config()
        };
        let voltage_gain_at = |config: OutputTransformerConfig, frequency: f64| -> f64 {
            let mut model = OutputTransformerModel::new(config, dt).expect("modelo");
            let omega = 2.0 * std::f64::consts::PI * frequency;
            let frames = (8.0 * rate / frequency) as usize;
            let (mut re, mut im) = (0.0, 0.0);
            for index in 0..frames {
                let vd = (omega * index as f64 * dt).sin();
                let id = model.differential_current(vd);
                let secondary = model.advance(vd, id).secondary_v;
                if index >= frames / 2 {
                    let angle = omega * index as f64 * dt;
                    re += secondary * angle.cos();
                    im -= secondary * angle.sin();
                }
            }
            re.hypot(im) / (frames as f64 / 4.0)
        };
        let corner_hz = (300.0 + 23.18 * 23.18 * 8.4) / (2.0 * std::f64::consts::PI * 0.012);
        assert!(
            (corner_hz - 64_000.0).abs() < 5_000.0,
            "esquina {corner_hz}"
        );
        let mid = voltage_gain_at(leakage_only, 1_000.0);
        let at_corner = voltage_gain_at(leakage_only, corner_hz);
        let ratio_db = 20.0 * (at_corner / mid).log10();
        // Tolerancia amplia: la fuga se integra con Euler atrás (memoria
        // acotada, véase `new`), que rota la impedancia ≈ −1 dB en la esquina
        // a esta tasa declarada (768 kHz). En la banda audible (≤20 kHz a 4×)
        // la desviación se cuantifica en `output_transformer_ac.rs`.
        assert!(
            (ratio_db + 3.0103).abs() < 1.3,
            "esquina HF {ratio_db:.2} dB"
        );

        // Con la red completa la banda audible queda plana.
        let mid_full = voltage_gain_at(config, 1_000.0);
        for frequency in [10_000.0, 20_000.0] {
            let deviation_db = 20.0 * (voltage_gain_at(config, frequency) / mid_full).log10();
            assert!(
                deviation_db.abs() < 1.0,
                "{frequency} Hz: {deviation_db:+.2} dB"
            );
        }

        // Con la excitación tipo pentodo (fuente + Rs, Rs = 2·rp del datasheet)
        // la banda audible queda plana y `Cp` domina por encima de ≈35 kHz.
        let source_ohms = 45_000.0;
        let thevenin_gain_at = |frequency: f64| -> f64 {
            let mut model = OutputTransformerModel::new(config, dt).expect("modelo");
            let omega = 2.0 * std::f64::consts::PI * frequency;
            let frames = (8.0 * rate / frequency) as usize;
            let (mut re, mut im) = (0.0, 0.0);
            for index in 0..frames {
                let vs = (omega * index as f64 * dt).sin();
                let (drop_at_zero, slope) = model.differential_drop(0.0);
                let id = (vs - drop_at_zero) / (source_ohms + slope);
                let (drop, _) = model.differential_drop(id);
                let secondary = model.advance(-drop, id).secondary_v;
                if index >= frames / 2 {
                    let angle = omega * index as f64 * dt;
                    re += secondary * angle.cos();
                    im -= secondary * angle.sin();
                }
            }
            re.hypot(im) / (frames as f64 / 4.0)
        };
        let mid = thevenin_gain_at(1_000.0);
        for frequency in [10_000.0, 20_000.0] {
            let deviation_db = 20.0 * (thevenin_gain_at(frequency) / mid).log10();
            assert!(
                deviation_db.abs() < 1.0,
                "{frequency} Hz: {deviation_db:+.2} dB"
            );
        }
    }

    #[test]
    fn reset_clears_all_state() {
        let mut model =
            OutputTransformerModel::new(linear_config(), 1.0 / 192_000.0).expect("modelo");
        for _ in 0..1_000 {
            let id = model.differential_current(5.0);
            model.advance(5.0, id);
        }
        model.reset();
        assert_eq!(model.secondary_v(), 0.0);
        assert_eq!(model.magnetizing_current_a(), 0.0);
        assert_eq!(model.reflected_current_a(), 0.0);
    }
}
