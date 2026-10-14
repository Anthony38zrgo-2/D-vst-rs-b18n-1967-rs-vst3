//! Red de tono pasiva de dos mandos (red James/Baxandall acoplada).
//!
//! # Netlist trazado del esquema 5/67 (`docs/CIRCUIT_TRACEABILITY.md`)
//!
//! Nodos: `SOURCE` (tensión de placa AC del segundo 6SL7 detrás de su
//! impedancia de salida `Ro`), `IN` (tras el acoplo de 0.022 µF), `B` (extremo
//! superior del pote de graves), `W` (cursor de graves / nodo común de la red),
//! `C` (extremo inferior del pote de graves), `T1` (extremo superior del pote
//! de agudos), `T2` (extremo inferior del pote de agudos), `OUT` (cursor de
//! agudos).
//!
//! - `Ro`: resistencia de Thevenin de la placa del 6SL7#2 (parámetro del
//!   motor; **una sola** fuente Norton en `SOURCE`, sin divisor externo).
//! - `0.022 µF`: acoplo `SOURCE`–`IN` (B18-003; antes vivía fuera del MNA con
//!   un paso alto de corte fijo y una pérdida de inserción duplicada).
//! - `220 kΩ`: IN–B (resistor serie de entrada).
//! - `0.00047 µF`: IN–T1 (paso de agudos).
//! - `0.001 µF`: B–W (red de graves).
//! - `0.01 µF`: C–W (red de graves).
//! - `POTE GRAVES 1 MΩ`: B–C con cursor en W (ley lineal).
//! - `22 kΩ`: C–masa (freno de la red de graves).
//! - `270 kΩ`: W–masa (recorte de graves documentado en foros técnicos
//!   como «270K from the bass control wiper to ground»).
//! - `120 kΩ`: W–T1 (enlace hacia la red de agudos).
//! - `POTE AGUDOS 1 MΩ`: T1–T2 con cursor en OUT (ley lineal).
//! - `0.0047 µF`: T2–masa (derivación de agudos).
//!
//! El resistor de mezcla de 270 kΩ por canal queda **fuera** de esta red.
//! La procedencia exacta de cada nodo está registrada en
//! `docs/CIRCUIT_TRACEABILITY.md`; los arrays del JSON no codifican conexiones.

use crate::circuit::CircuitDefinition;
use crate::numerics::discretization::TptCapacitor;
use crate::numerics::mna::{Complex, ComplexMna, RealMna};
use std::fmt;

/// Nodo de masa del solver.
const GND: usize = 0;
const NODE_SOURCE: usize = 1;
const NODE_IN: usize = 2;
const NODE_B: usize = 3;
const NODE_W: usize = 4;
const NODE_C: usize = 5;
const NODE_T1: usize = 6;
const NODE_T2: usize = 7;
const NODE_OUT: usize = 8;
const NODES: usize = 8;

/// Posiciones de los mandos de tono (`0.0..=1.0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToneStackControls {
    pub bass: f32,
    pub treble: f32,
}

impl Default for ToneStackControls {
    fn default() -> Self {
        Self {
            bass: 0.5,
            treble: 0.5,
        }
    }
}

/// Valores físicos de la red, extraídos del `CircuitDefinition` por valor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToneStackDesign {
    pub series_resistor_ohms: f64,
    pub bass_top_capacitor_farads: f64,
    pub bass_bottom_capacitor_farads: f64,
    pub bass_stop_resistor_ohms: f64,
    pub bass_cut_resistor_ohms: f64,
    pub mid_resistor_ohms: f64,
    pub treble_series_capacitor_farads: f64,
    pub treble_shunt_capacitor_farads: f64,
    pub bass_pot_ohms: f64,
    pub treble_pot_ohms: f64,
}

/// Error de construcción de la red.
#[derive(Debug, Clone, PartialEq)]
pub enum ToneStackError {
    MissingValue { what: &'static str, expected: f64 },
    InvalidValue { what: &'static str, value: f64 },
}

impl fmt::Display for ToneStackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingValue { what, expected } => {
                write!(f, "tone_stack: falta {what} con valor esperado {expected}")
            }
            Self::InvalidValue { what, value } => {
                write!(f, "tone_stack: {what} no válido ({value})")
            }
        }
    }
}

impl std::error::Error for ToneStackError {}

impl ToneStackDesign {
    /// Construye el diseño localizando los valores en las listas del JSON.
    ///
    /// La búsqueda es por valor (no por posición): el JSON advierte que el
    /// orden de los arrays no codifica conexiones.
    pub fn from_circuit(circuit: &CircuitDefinition) -> Result<Self, ToneStackError> {
        let resistors = &circuit.tone_stack.printed_board_resistors_ohms;
        let capacitors = &circuit.tone_stack.printed_board_capacitors_farads;
        let additional = &circuit.tone_stack.additional_resistors_visible_ohms;

        let series_resistor_ohms = pick(resistors, 220_000.0, "resistor serie 220 kΩ")?;
        let bass_stop_resistor_ohms = pick(resistors, 22_000.0, "resistor de freno 22 kΩ")?;
        let mid_resistor_ohms = pick(additional, 120_000.0, "resistor medio 120 kΩ")?;
        let bass_cut_resistor_ohms = pick(additional, 270_000.0, "resistor de recorte 270 kΩ")?;
        let bass_top_capacitor_farads = pick(capacitors, 1e-9, "capacitor 0.001 µF")?;
        let bass_bottom_capacitor_farads = pick(capacitors, 1e-8, "capacitor 0.01 µF")?;
        let treble_series_capacitor_farads = pick(capacitors, 4.7e-10, "capacitor 0.00047 µF")?;
        let treble_shunt_capacitor_farads = pick(capacitors, 4.7e-9, "capacitor 0.0047 µF")?;

        let bass_pot_ohms = circuit.channel.bass_pot.max_ohms;
        let treble_pot_ohms = circuit.channel.treble_pot.max_ohms;

        for (what, value) in [
            ("resistor serie de entrada", series_resistor_ohms),
            ("freno de graves", bass_stop_resistor_ohms),
            ("resistor medio", mid_resistor_ohms),
            ("recorte de graves", bass_cut_resistor_ohms),
            ("capacitor de graves superior", bass_top_capacitor_farads),
            ("capacitor de graves inferior", bass_bottom_capacitor_farads),
            ("capacitor serie de agudos", treble_series_capacitor_farads),
            ("capacitor de agudos a masa", treble_shunt_capacitor_farads),
        ] {
            if !(value.is_finite() && value > 0.0) {
                return Err(ToneStackError::InvalidValue { what, value });
            }
        }

        Ok(Self {
            series_resistor_ohms,
            bass_top_capacitor_farads,
            bass_bottom_capacitor_farads,
            bass_stop_resistor_ohms,
            bass_cut_resistor_ohms,
            mid_resistor_ohms,
            treble_series_capacitor_farads,
            treble_shunt_capacitor_farads,
            bass_pot_ohms,
            treble_pot_ohms,
        })
    }

    /// Función de transferencia `OUT/IN` a `freq_hz`.
    ///
    /// `source_ohms` es la impedancia de la etapa que excita la red y
    /// `load_ohms` la carga vista a la salida (resistor de mezcla y etapa
    /// siguiente). La fuente se modela como Norton unitario, así que el
    /// resultado es `V_out / V_in` con `V_in` ya cargado por `source_ohms`.
    #[must_use]
    pub fn frequency_response(
        &self,
        controls: ToneStackControls,
        freq_hz: f64,
        source_ohms: f64,
        load_ohms: f64,
    ) -> Option<Complex> {
        // Mapa de nodos local y compacto del tono aislado (sin `SOURCE`), para
        // que el fixture histórico no dependa del nodo añadido en B18-003.
        const IN: usize = 1;
        const B: usize = 2;
        const W: usize = 3;
        const C: usize = 4;
        const T1: usize = 5;
        const T2: usize = 6;
        const OUT: usize = 7;
        const TONE_ONLY_NODES: usize = 7;

        let omega = 2.0 * std::f64::consts::PI * freq_hz;
        let (r_bw, r_wc) = bass_pot_split(self.bass_pot_ohms, controls.bass);
        let (r_t_out, r_out_t2) = treble_pot_split(self.treble_pot_ohms, controls.treble);

        let mut mna = ComplexMna::new(TONE_ONLY_NODES);
        mna.add_resistor(IN, GND, source_ohms.max(1e-3));
        mna.add_resistor(IN, B, self.series_resistor_ohms);
        mna.add_capacitor(IN, T1, self.treble_series_capacitor_farads, omega);
        mna.add_capacitor(B, W, self.bass_top_capacitor_farads, omega);
        mna.add_capacitor(C, W, self.bass_bottom_capacitor_farads, omega);
        mna.add_resistor(B, W, r_bw);
        mna.add_resistor(W, C, r_wc);
        mna.add_resistor(C, GND, self.bass_stop_resistor_ohms);
        mna.add_resistor(W, GND, self.bass_cut_resistor_ohms);
        mna.add_resistor(W, T1, self.mid_resistor_ohms);
        mna.add_resistor(T1, OUT, r_t_out);
        mna.add_resistor(OUT, T2, r_out_t2);
        mna.add_capacitor(T2, GND, self.treble_shunt_capacitor_farads, omega);
        mna.add_resistor(OUT, GND, load_ohms.max(1e-3));
        mna.add_current_injection(IN, Complex::ONE);

        let solution = mna.solve()?;
        Some(solution[OUT] / solution[IN])
    }

    /// Función de transferencia **con el acoplo de entrada real** (B18-003):
    /// `V_out / V_source`, donde `V_source` es la tensión de placa AC del
    /// 6SL7#2 en circuito abierto y `source_ohms` su impedancia de Thevenin.
    ///
    /// Topología: fuente de 1 V tras `source_ohms` en `SOURCE`, capacitor de
    /// acoplo `coupling_farads` entre `SOURCE` e `IN`, y la misma red pasiva
    /// que [`Self::frequency_response`]. La API de tono aislado se conserva sin
    /// cambios para no alterar el significado de los fixtures antiguos.
    #[must_use]
    pub fn frequency_response_with_input_coupling(
        &self,
        controls: ToneStackControls,
        freq_hz: f64,
        source_ohms: f64,
        coupling_farads: f64,
        load_ohms: f64,
    ) -> Option<Complex> {
        let omega = 2.0 * std::f64::consts::PI * freq_hz;
        let (r_bw, r_wc) = bass_pot_split(self.bass_pot_ohms, controls.bass);
        let (r_t_out, r_out_t2) = treble_pot_split(self.treble_pot_ohms, controls.treble);
        let source_ohms = source_ohms.max(1e-3);

        let mut mna = ComplexMna::new(NODES);
        // Fuente de Thevenin de 1 V tras `source_ohms` (Norton equivalente):
        // la respuesta es `V_out / 1 V`.
        mna.add_resistor(NODE_SOURCE, GND, source_ohms);
        mna.add_current_injection(NODE_SOURCE, Complex::new(1.0 / source_ohms, 0.0));
        mna.add_capacitor(NODE_SOURCE, NODE_IN, coupling_farads, omega);
        mna.add_resistor(NODE_IN, NODE_B, self.series_resistor_ohms);
        mna.add_capacitor(NODE_IN, NODE_T1, self.treble_series_capacitor_farads, omega);
        mna.add_capacitor(NODE_B, NODE_W, self.bass_top_capacitor_farads, omega);
        mna.add_capacitor(NODE_C, NODE_W, self.bass_bottom_capacitor_farads, omega);
        mna.add_resistor(NODE_B, NODE_W, r_bw);
        mna.add_resistor(NODE_W, NODE_C, r_wc);
        mna.add_resistor(NODE_C, GND, self.bass_stop_resistor_ohms);
        mna.add_resistor(NODE_W, GND, self.bass_cut_resistor_ohms);
        mna.add_resistor(NODE_W, NODE_T1, self.mid_resistor_ohms);
        mna.add_resistor(NODE_T1, NODE_OUT, r_t_out);
        mna.add_resistor(NODE_OUT, NODE_T2, r_out_t2);
        mna.add_capacitor(NODE_T2, GND, self.treble_shunt_capacitor_farads, omega);
        mna.add_resistor(NODE_OUT, GND, load_ohms.max(1e-3));

        let solution = mna.solve()?;
        Some(solution[NODE_OUT])
    }
}

/// Procesador en tiempo real de la red de tono (MNA + capacitores TPT).
///
/// Incluye el acoplo de entrada de 0.022 µF entre `SOURCE` (placa AC del
/// 6SL7#2 detrás de su impedancia de salida) e `IN` (B18-003): la continua
/// queda bloqueada por el propio capacitor de acoplo y la carga de la red
/// depende de las posiciones de Bass/Treble.
///
/// Sin asignaciones dentro de [`Self::process_sample`]; los valores de los
/// potes invalidan la factorización solo cuando cambian mediante [`Self::set_controls`].
#[derive(Debug, Clone)]
pub struct ToneStackProcessor {
    design: ToneStackDesign,
    source_ohms: f64,
    load_ohms: f64,
    controls: ToneStackControls,
    mna: RealMna,
    matrix_dirty: bool,
    solution: [f64; NODES + 1],
    cap_input_coupling: TptCapacitor,
    cap_treble_series: TptCapacitor,
    cap_bass_top: TptCapacitor,
    cap_bass_bottom: TptCapacitor,
    cap_treble_shunt: TptCapacitor,
}

impl ToneStackProcessor {
    #[must_use]
    pub fn new(
        design: ToneStackDesign,
        source_ohms: f64,
        load_ohms: f64,
        coupling_farads: f64,
        sample_rate_hz: f64,
        oversampling: u8,
    ) -> Self {
        let dt = 1.0 / (sample_rate_hz * f64::from(oversampling.max(1)));
        Self {
            design,
            source_ohms: source_ohms.max(1e-3),
            load_ohms: load_ohms.max(1e-3),
            controls: ToneStackControls::default(),
            mna: RealMna::new(NODES),
            matrix_dirty: true,
            solution: [0.0; NODES + 1],
            cap_input_coupling: TptCapacitor::new(coupling_farads, dt),
            cap_treble_series: TptCapacitor::new(design.treble_series_capacitor_farads, dt),
            cap_bass_top: TptCapacitor::new(design.bass_top_capacitor_farads, dt),
            cap_bass_bottom: TptCapacitor::new(design.bass_bottom_capacitor_farads, dt),
            cap_treble_shunt: TptCapacitor::new(design.treble_shunt_capacitor_farads, dt),
        }
    }

    pub fn set_controls(&mut self, controls: ToneStackControls) {
        let controls = ToneStackControls {
            bass: controls.bass.clamp(0.0, 1.0),
            treble: controls.treble.clamp(0.0, 1.0),
        };
        self.matrix_dirty |= self.controls != controls;
        self.controls = controls;
    }

    pub fn reset(&mut self) {
        self.cap_input_coupling.reset();
        self.cap_treble_series.reset();
        self.cap_bass_top.reset();
        self.cap_bass_bottom.reset();
        self.cap_treble_shunt.reset();
        self.solution = [0.0; NODES + 1];
    }

    /// Restaura el estado desde otro procesador **sin asignar memoria**
    /// (apto para el hilo de audio; el MNA reutiliza sus buffers).
    pub fn restore_from(&mut self, other: &Self) {
        self.design = other.design;
        self.source_ohms = other.source_ohms;
        self.load_ohms = other.load_ohms;
        self.controls = other.controls;
        self.mna.restore_from(&other.mna);
        self.matrix_dirty = other.matrix_dirty;
        self.solution = other.solution;
        self.cap_input_coupling = other.cap_input_coupling;
        self.cap_treble_series = other.cap_treble_series;
        self.cap_bass_top = other.cap_bass_top;
        self.cap_bass_bottom = other.cap_bass_bottom;
        self.cap_treble_shunt = other.cap_treble_shunt;
    }

    /// Procesa un sample de la **tensión de placa AC del 6SL7#2** (deviation
    /// respecto del reposo). La red interna resuelve el acoplo de 0.022 µF y
    /// devuelve la tensión del cursor de agudos.
    pub fn process_sample(&mut self, input: f64) -> f64 {
        let mna = &mut self.mna;
        if self.matrix_dirty {
            let (r_bw, r_wc) = bass_pot_split(self.design.bass_pot_ohms, self.controls.bass);
            let (r_t_out, r_out_t2) =
                treble_pot_split(self.design.treble_pot_ohms, self.controls.treble);

            mna.clear();
            // Fuente de Thevenin `Ro` en SOURCE; el acoplo de 22 nF une SOURCE
            // con IN (una sola caída de tensión, sin divisor externo).
            mna.add_conductance(NODE_SOURCE, GND, 1.0 / self.source_ohms);
            mna.add_conductance(NODE_IN, NODE_B, 1.0 / self.design.series_resistor_ohms);
            mna.add_conductance(NODE_B, NODE_W, 1.0 / r_bw);
            mna.add_conductance(NODE_W, NODE_C, 1.0 / r_wc);
            mna.add_conductance(NODE_C, GND, 1.0 / self.design.bass_stop_resistor_ohms);
            mna.add_conductance(NODE_W, GND, 1.0 / self.design.bass_cut_resistor_ohms);
            mna.add_conductance(NODE_W, NODE_T1, 1.0 / self.design.mid_resistor_ohms);
            mna.add_conductance(NODE_T1, NODE_OUT, 1.0 / r_t_out);
            mna.add_conductance(NODE_OUT, NODE_T2, 1.0 / r_out_t2);
            mna.add_conductance(NODE_OUT, GND, 1.0 / self.load_ohms);

            for (a, b, cap) in [
                (NODE_SOURCE, NODE_IN, &self.cap_input_coupling),
                (NODE_IN, NODE_T1, &self.cap_treble_series),
                (NODE_B, NODE_W, &self.cap_bass_top),
                (NODE_C, NODE_W, &self.cap_bass_bottom),
                (NODE_T2, GND, &self.cap_treble_shunt),
            ] {
                mna.add_conductance(a, b, cap.conductance());
            }
            self.matrix_dirty = false;
        }
        mna.clear_rhs();

        for (a, b, cap) in [
            (NODE_SOURCE, NODE_IN, &self.cap_input_coupling),
            (NODE_IN, NODE_T1, &self.cap_treble_series),
            (NODE_B, NODE_W, &self.cap_bass_top),
            (NODE_C, NODE_W, &self.cap_bass_bottom),
            (NODE_T2, GND, &self.cap_treble_shunt),
        ] {
            let ieq = cap.companion_current();
            mna.add_current_injection(a, ieq);
            mna.add_current_injection(b, -ieq);
        }

        mna.add_current_injection(NODE_SOURCE, input / self.source_ohms);

        if !mna.solve_into(&mut self.solution) {
            self.reset();
            return 0.0;
        }

        let voltage = |node: usize| self.solution[node];
        self.cap_input_coupling
            .update(voltage(NODE_SOURCE) - voltage(NODE_IN));
        self.cap_treble_series
            .update(voltage(NODE_IN) - voltage(NODE_T1));
        self.cap_bass_top.update(voltage(NODE_B) - voltage(NODE_W));
        self.cap_bass_bottom
            .update(voltage(NODE_C) - voltage(NODE_W));
        self.cap_treble_shunt
            .update(voltage(NODE_T2) - voltage(GND));

        voltage(NODE_OUT)
    }
}

/// División del pote de graves: `(R_BW, R_WC)` con cursor `1.0` en B.
#[must_use]
pub fn bass_pot_split(pot_ohms: f64, position: f32) -> (f64, f64) {
    let p = f64::from(position.clamp(0.0, 1.0)).clamp(1e-6, 1.0 - 1e-6);
    ((1.0 - p) * pot_ohms, p * pot_ohms)
}

/// División del pote de agudos: `(R_T1_OUT, R_OUT_T2)` con cursor `1.0` en T1.
#[must_use]
pub fn treble_pot_split(pot_ohms: f64, position: f32) -> (f64, f64) {
    let p = f64::from(position.clamp(0.0, 1.0)).clamp(1e-6, 1.0 - 1e-6);
    ((1.0 - p) * pot_ohms, p * pot_ohms)
}

fn pick(list: &[f64], target: f64, what: &'static str) -> Result<f64, ToneStackError> {
    list.iter()
        .copied()
        .find(|value| (value - target).abs() / target < 1e-6)
        .ok_or(ToneStackError::MissingValue {
            what,
            expected: target,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::test_circuit;

    /// Acoplo de placa del 6SL7#2 (0.022 µF) del esquema 5/67.
    const COUPLING_FARADS: f64 = 2.2e-8;
    /// Impedancia de fuente de los fixtures (misma que la referencia SPICE).
    const FIXTURE_SOURCE_OHMS: f64 = 100_000.0;
    const FIXTURE_LOAD_OHMS: f64 = 270_000.0;

    fn design() -> ToneStackDesign {
        ToneStackDesign::from_circuit(&test_circuit()).expect("valores presentes en el JSON")
    }

    #[test]
    fn cached_tone_matches_rebuilt_network_during_automation_and_reset() {
        let mut cached =
            ToneStackProcessor::new(design(), 50_000.0, 270_000.0, COUPLING_FARADS, 48_000.0, 1);
        let mut rebuilt = cached.clone();
        for i in 0..4096 {
            if i % 256 == 0 {
                let controls = ToneStackControls {
                    bass: (i % 1024) as f32 / 1023.0,
                    treble: 1.0 - (i % 1024) as f32 / 1023.0,
                };
                cached.set_controls(controls);
                rebuilt.set_controls(controls);
            }
            if i == 2048 {
                cached.reset();
                rebuilt.reset();
            }
            rebuilt.matrix_dirty = true;
            let input = (i as f64 * 0.13).sin();
            assert_eq!(cached.process_sample(input), rebuilt.process_sample(input));
        }
    }

    #[test]
    fn design_extracts_documented_values() {
        let design = design();
        assert_eq!(design.series_resistor_ohms, 220_000.0);
        assert_eq!(design.bass_stop_resistor_ohms, 22_000.0);
        assert_eq!(design.mid_resistor_ohms, 120_000.0);
        assert_eq!(design.bass_cut_resistor_ohms, 270_000.0);
        assert_eq!(design.treble_series_capacitor_farads, 4.7e-10);
        assert_eq!(design.treble_shunt_capacitor_farads, 4.7e-9);
        assert_eq!(design.bass_pot_ohms, 1_000_000.0);
    }

    #[test]
    fn frequency_response_is_finite_and_bounded() {
        let design = design();
        for bass in [0.0_f32, 0.5, 1.0] {
            for treble in [0.0_f32, 0.5, 1.0] {
                for freq in [20.0, 100.0, 1_000.0, 10_000.0, 20_000.0] {
                    let h = design
                        .frequency_response(
                            ToneStackControls { bass, treble },
                            freq,
                            100_000.0,
                            270_000.0,
                        )
                        .expect("resoluble");
                    let db = h.magnitude_db();
                    assert!(db.is_finite(), "dB no finito a {freq} Hz");
                    assert!(db < 12.0, "ganancia excesiva ({db} dB) a {freq} Hz");
                    assert!(db > -80.0, "atenuación excesiva ({db} dB) a {freq} Hz");
                }
            }
        }
    }

    #[test]
    fn bass_and_treble_move_in_expected_direction() {
        let design = design();
        let at = |bass, treble, freq| {
            design
                .frequency_response(
                    ToneStackControls { bass, treble },
                    freq,
                    100_000.0,
                    270_000.0,
                )
                .expect("resoluble")
                .magnitude_db()
        };
        let bass_boost = at(1.0, 0.5, 40.0) - at(0.0, 0.5, 40.0);
        let treble_boost = at(0.5, 1.0, 5_000.0) - at(0.5, 0.0, 5_000.0);
        assert!(
            bass_boost > 6.0,
            "rango de graves insuficiente: {bass_boost} dB"
        );
        assert!(
            treble_boost > 10.0,
            "rango de agudos insuficiente: {treble_boost} dB"
        );
    }

    #[test]
    fn processor_sine_is_finite_and_settles() {
        let design = design();
        let sample_rate = 48_000.0;
        let mut processor = ToneStackProcessor::new(
            design,
            100_000.0,
            270_000.0,
            COUPLING_FARADS,
            sample_rate,
            1,
        );
        processor.set_controls(ToneStackControls {
            bass: 0.5,
            treble: 0.5,
        });
        let mut peak = 0.0_f64;
        for index in 0..48_000 {
            let t = index as f64 / sample_rate;
            let input = (2.0 * std::f64::consts::PI * 440.0 * t).sin();
            let output = processor.process_sample(input);
            assert!(output.is_finite());
            peak = peak.max(output.abs());
        }
        assert!(peak > 0.0, "la red no dejó pasar señal");
    }

    #[test]
    fn processor_output_is_block_size_invariant() {
        let design = design();
        let input: Vec<f64> = (0..4096).map(|i| (i as f64 * 0.01).sin() * 0.5).collect();

        let mut whole =
            ToneStackProcessor::new(design, 100_000.0, 270_000.0, COUPLING_FARADS, 48_000.0, 1);
        let mut chunked =
            ToneStackProcessor::new(design, 100_000.0, 270_000.0, COUPLING_FARADS, 48_000.0, 1);
        whole.set_controls(ToneStackControls {
            bass: 0.3,
            treble: 0.7,
        });
        chunked.set_controls(ToneStackControls {
            bass: 0.3,
            treble: 0.7,
        });

        let mut out_whole = Vec::with_capacity(input.len());
        for &x in &input {
            out_whole.push(whole.process_sample(x));
        }
        let mut out_chunked = Vec::with_capacity(input.len());
        for chunk in input.chunks(257) {
            for &x in chunk {
                out_chunked.push(chunked.process_sample(x));
            }
        }
        assert_eq!(out_whole, out_chunked);
    }

    /// El acoplo de 0.022 µF bloquea la continua: ante una fuente constante la
    /// salida tiende a cero (antes el paso alto externo de corte fijo hacía esa
    /// función fuera del MNA).
    #[test]
    fn coupled_input_blocks_dc() {
        let mut processor = ToneStackProcessor::new(
            design(),
            FIXTURE_SOURCE_OHMS,
            FIXTURE_LOAD_OHMS,
            COUPLING_FARADS,
            48_000.0,
            1,
        );
        processor.set_controls(ToneStackControls {
            bass: 0.5,
            treble: 0.5,
        });
        let mut output = 0.0;
        for _ in 0..192_000 {
            output = processor.process_sample(1.0);
        }
        assert!(output.abs() < 1e-3, "continua no bloqueada: {output}");
    }

    /// El procesador temporal reproduce la respuesta compleja del análisis AC
    /// con el mismo acoplo y carga (valida la discretización TPT).
    #[test]
    fn coupled_processor_matches_complex_response_within_discretization_error() {
        let design = design();
        let controls = ToneStackControls {
            bass: 0.7,
            treble: 0.3,
        };
        let rate = 192_000.0;
        let frequency = 1_000.0;
        let mut processor = ToneStackProcessor::new(
            design,
            FIXTURE_SOURCE_OHMS,
            FIXTURE_LOAD_OHMS,
            COUPLING_FARADS,
            rate,
            1,
        );
        processor.set_controls(controls);

        let frames = 96_000;
        let settle = frames / 2;
        let omega = 2.0 * std::f64::consts::PI * frequency / rate;
        let (mut re, mut im) = (0.0, 0.0);
        for index in 0..frames {
            // Referencia coseno para que el armónico caiga en fase 0 (la
            // ventana contiene un número entero de períodos a 1 kHz/192 kHz).
            let input = (omega * index as f64).cos();
            let output = processor.process_sample(input);
            if index >= settle {
                let angle = omega * index as f64;
                re += output * angle.cos();
                im -= output * angle.sin();
            }
        }
        let magnitude = re.hypot(im) / ((frames - settle) as f64 / 2.0);
        let measured_db = 20.0 * magnitude.log10();
        let measured_phase = im.atan2(re).to_degrees();

        let expected = design
            .frequency_response_with_input_coupling(
                controls,
                frequency,
                FIXTURE_SOURCE_OHMS,
                COUPLING_FARADS,
                FIXTURE_LOAD_OHMS,
            )
            .expect("red resoluble");
        assert!(
            (measured_db - expected.magnitude_db()).abs() < 0.01,
            "magnitud temporal {} dB vs AC {} dB",
            measured_db,
            expected.magnitude_db()
        );
        let phase_error = (measured_phase - expected.phase_deg()).abs();
        assert!(phase_error < 0.2, "fase {phase_error}°");
    }

    /// Construir a `rate × os` es equivalente a construir a la tasa efectiva.
    #[test]
    fn oversampling_and_rate_are_equivalent_through_dt() {
        let mut at_48k_4x = ToneStackProcessor::new(
            design(),
            FIXTURE_SOURCE_OHMS,
            FIXTURE_LOAD_OHMS,
            COUPLING_FARADS,
            48_000.0,
            4,
        );
        let mut at_192k_1x = ToneStackProcessor::new(
            design(),
            FIXTURE_SOURCE_OHMS,
            FIXTURE_LOAD_OHMS,
            COUPLING_FARADS,
            192_000.0,
            1,
        );
        let controls = ToneStackControls {
            bass: 0.4,
            treble: 0.6,
        };
        at_48k_4x.set_controls(controls);
        at_192k_1x.set_controls(controls);
        for index in 0..8_192 {
            let input = (index as f64 * 0.07).sin();
            assert_eq!(
                at_48k_4x.process_sample(input),
                at_192k_1x.process_sample(input)
            );
        }
    }

    /// Con el acoplo real, los mandos siguen moviendo graves/agudos en la
    /// dirección documentada (el corte ahora depende de las posiciones).
    #[test]
    fn coupled_controls_span_documented_directions() {
        let design = design();
        let at = |bass: f32, treble: f32, frequency: f64| {
            design
                .frequency_response_with_input_coupling(
                    ToneStackControls { bass, treble },
                    frequency,
                    FIXTURE_SOURCE_OHMS,
                    COUPLING_FARADS,
                    FIXTURE_LOAD_OHMS,
                )
                .expect("red resoluble")
                .magnitude_db()
        };
        let bass_boost = at(1.0, 0.5, 40.0) - at(0.0, 0.5, 40.0);
        let treble_boost = at(0.5, 1.0, 5_000.0) - at(0.5, 0.0, 5_000.0);
        assert!(
            bass_boost > 6.0,
            "rango de graves insuficiente con acoplo: {bass_boost} dB"
        );
        assert!(
            treble_boost > 8.0,
            "rango de agudos insuficiente con acoplo: {treble_boost} dB"
        );
    }
}
