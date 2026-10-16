//! B18-013: contraste AC del OT-471 lineal contra ngspice.
//!
//! El golden `ot471_linear_spice_cp15n.csv` lo genera
//! `scripts/spice/output_transformer_reference.py --cp 1.5e-9` con ngspice 47
//! sobre `scripts/spice/b18n_ot471_linear.cir` (equivalente diferencial con
//! Lm=28.7 H, Lleak=12 mH, Cp=1,5 nF corregido en B18-009, R_core=500 Ω,
//! DCR 300/0.4 Ω, N8=23.18, carga 8 Ω y fuente Thevenin Rs=45 kΩ = 2·rp de la
//! 7027A del datasheet). El script comprueba el netlist contra una
//! implementación analítica independiente antes de escribir el CSV. El golden
//! histórico con Cp=100 pF se conserva sin cambios.
//!
//! El motor integra con Euler atrás los dos inductores (memoria acotada,
//! robustez del solve) y TPT el condensador; la desviación frente a la
//! referencia continua queda acotada en la banda audible con las tolerancias
//! declaradas aquí (medidas a la tasa de proceso 4× de 48 kHz).

use b18n_dsp::stages::output_transformer::{OutputTransformerConfig, OutputTransformerModel};
use b18n_dsp::stages::speaker_impedance::ElectricalLoadConfig;

const GOLDEN: &str = include_str!(
    "../../../assets/fixtures/reference/output_transformer/ot471_linear_spice_cp15n.csv"
);

const SAMPLE_RATE_HZ: f64 = 48_000.0;
const OVERSAMPLING: f64 = 4.0;
/// Tasa de proceso del plugin (4× de 48 kHz).
const PROCESS_RATE_HZ: f64 = SAMPLE_RATE_HZ * OVERSAMPLING;
/// Tasa alta para aislar la topología del warping de discretización.
const TOPOLOGY_RATE_HZ: f64 = 768_000.0;
/// Fuente Thevenin del contraste: 2·rp(7027A, datasheet RCA clase A1).
const SOURCE_OHMS: f64 = 45_000.0;

fn model_config() -> OutputTransformerConfig {
    let circuit = b18n_config::load_reference().expect("JSON + calibración");
    OutputTransformerConfig {
        primary_plate_to_plate_ohms_nominal: circuit
            .output_transformer
            .primary_plate_to_plate_ohms_nominal,
        turns_ratio_8ohm: circuit.output_transformer.turns_ratio_primary_to_8ohm,
        turns_ratio_16ohm: circuit.output_transformer.turns_ratio_primary_to_16ohm,
        dc_primary_winding_ohms: circuit.output_transformer.dc_primary_winding_ohms,
        dc_secondary_8ohm_winding_ohms: circuit.output_transformer.dc_secondary_8ohm_winding_ohms,
        load: ElectricalLoadConfig::Resistive {
            ohms: circuit.speaker.nominal_load_ohms,
        },
        magnetizing_inductance_h: circuit.output_transformer.winding_inductance_h,
        leakage_inductance_h: circuit.output_transformer.leakage_inductance_h,
        parasitic_capacitance_f: circuit.output_transformer.parasitic_capacitance_f,
        core_loss_ohms: circuit.output_transformer.core_loss_ohms,
        magnetizing_current_limit_a: circuit.output_transformer.magnetizing_current_limit_a,
    }
}

#[test]
fn secondary_port_predicts_the_same_sample_without_advancing_history() {
    let circuit = b18n_config::load_reference().expect("reference");
    for load in [
        ElectricalLoadConfig::Resistive { ohms: 8.0 },
        ElectricalLoadConfig::Resistive { ohms: 16.0 },
        ElectricalLoadConfig::LumpedEquivalent(circuit.speaker.lumped_equivalent.expect("RLC")),
    ] {
        let mut config = model_config();
        config.load = load;
        let mut model = OutputTransformerModel::new(config, 1.0 / 192_000.0).expect("OT");
        for index in 0..2000 {
            let before = model;
            let (offset, gain) = model.secondary_voltage_port();
            assert_eq!(model, before, "a Newton trial must not advance histories");
            let voltage = 40.0 * (index as f64 * 0.03).sin();
            let current = model.differential_current(voltage);
            let actual = model.advance(voltage, current).secondary_v;
            assert!((actual - offset - gain * voltage).abs() < 1e-9);
        }
    }
}

/// Respuesta compleja `vsec/vs` del modelo a `frequency_hz`, con la misma
/// excitación Thevenin del golden (fuente interna de 1 V). La fuente se
/// escribe como coseno, la convención de fase del `.ac` de SPICE.
fn model_transfer(
    config: OutputTransformerConfig,
    frequency_hz: f64,
    processing_rate: f64,
) -> (f64, f64) {
    let dt = 1.0 / processing_rate;
    let mut model = OutputTransformerModel::new(config, dt).expect("modelo del OT");
    let omega = 2.0 * std::f64::consts::PI * frequency_hz;
    let frames = (8.0 * processing_rate / frequency_hz) as usize;
    let (mut real, mut imag) = (0.0_f64, 0.0_f64);
    for index in 0..frames {
        let angle = omega * index as f64 * dt;
        let vs = angle.cos();
        let (drop_at_zero, slope) = model.differential_drop(0.0);
        let id = (vs - drop_at_zero) / (SOURCE_OHMS + slope);
        let (drop, _) = model.differential_drop(id);
        let secondary = model.advance(-drop, id).secondary_v;
        if index >= frames / 2 {
            real += secondary * angle.cos();
            imag -= secondary * angle.sin();
        }
    }
    let norm = frames as f64 / 4.0;
    (real / norm, imag / norm)
}

struct GoldenPoint {
    frequency_hz: f64,
    db: f64,
    phase_deg: f64,
}

fn golden_points() -> Vec<GoldenPoint> {
    let mut points = Vec::new();
    for line in GOLDEN.lines() {
        if line.starts_with('#') || line.starts_with("frequency_hz") {
            continue;
        }
        let mut fields = line.split(',');
        let frequency_hz: f64 = fields
            .next()
            .expect("frecuencia")
            .trim()
            .parse()
            .expect("número");
        let db: f64 = fields.next().expect("dB").trim().parse().expect("número");
        let phase_deg: f64 = fields.next().expect("fase").trim().parse().expect("número");
        points.push(GoldenPoint {
            frequency_hz,
            db,
            phase_deg,
        });
    }
    assert!(points.len() > 80, "golden incompleto: {}", points.len());
    points
}

/// Desviación compleja máxima en `[20 Hz, 20 kHz]` (magnitud y fase envueltas).
fn worst_deviation(band_low_hz: f64, band_high_hz: f64, processing_rate: f64) -> (f64, f64, f64) {
    let config = model_config();
    let (mut worst_db, mut worst_phase_deg, mut at_hz) = (0.0_f64, 0.0_f64, 0.0_f64);
    for point in golden_points() {
        if !(band_low_hz..=band_high_hz).contains(&point.frequency_hz) {
            continue;
        }
        let (real, imag) = model_transfer(config, point.frequency_hz, processing_rate);
        let magnitude = real.hypot(imag);
        let model_db = 20.0 * magnitude.log10();
        let model_phase_deg = imag.atan2(real).to_degrees();
        let db_error = model_db - point.db;
        let mut phase_error = model_phase_deg - point.phase_deg;
        while phase_error > 180.0 {
            phase_error -= 360.0;
        }
        while phase_error < -180.0 {
            phase_error += 360.0;
        }
        if db_error.abs() > worst_db {
            worst_db = db_error.abs();
            at_hz = point.frequency_hz;
        }
        worst_phase_deg = worst_phase_deg.max(phase_error.abs());
    }
    (worst_db, worst_phase_deg, at_hz)
}

/// Magnitud y fase del modelo frente al golden SPICE en la banda audible,
/// evaluadas a una tasa donde el warping de discretización es despreciable
/// (la desviación a la tasa de proceso se mide aparte).
#[test]
fn magnitude_and_phase_match_spice_in_the_audio_band() {
    let (worst_db, worst_phase_deg, at_hz) = worst_deviation(20.0, 20_000.0, TOPOLOGY_RATE_HZ);
    println!(
        "peor desviación en banda audible (768 kHz): {worst_db:.3} dB @ {at_hz:.0} Hz, \
         {worst_phase_deg:.2}°"
    );
    assert!(
        worst_db < 0.5,
        "magnitud fuera de tolerancia: {worst_db:.3} dB @ {at_hz:.0} Hz"
    );
    assert!(
        worst_phase_deg < 10.0,
        "fase fuera de tolerancia: {worst_phase_deg:.2}° @ {at_hz:.0} Hz"
    );
}

/// A la tasa de proceso (192 kHz) el modelo desvía del continuo por el warping
/// TPT del condensador parásito; se mide y acota explícitamente, sin ocultarlo
/// aflojando la puerta de topología.
#[test]
fn process_rate_high_frequency_deviation_is_bounded() {
    let (worst_db, worst_phase_deg, at_hz) = worst_deviation(10_000.0, 20_000.0, PROCESS_RATE_HZ);
    println!(
        "desviación a tasa de proceso (192 kHz, 10–20 kHz): {worst_db:.3} dB @ {at_hz:.0} Hz, \
         {worst_phase_deg:.2}°"
    );
    assert!(
        worst_db < 1.0,
        "magnitud a tasa de proceso fuera de cota: {worst_db:.3} dB @ {at_hz:.0} Hz"
    );
    assert!(worst_phase_deg < 10.0);
}

/// La caída sub-20 Hz queda gobernada por `Lm`: a 25 Hz el modelo coincide
/// con el golden (la esquina efectiva la fijan `Lm`, `R_core` y la fuente
/// Thevenin, ≈ −2,6 dB respecto de 200 Hz en el propio netlist SPICE).
#[test]
fn low_frequency_corner_matches_the_golden() {
    let config = model_config();
    let at = |frequency_hz: f64| -> f64 {
        let (real, imag) = model_transfer(config, frequency_hz, PROCESS_RATE_HZ);
        20.0 * real.hypot(imag).log10()
    };
    let golden_db = |target: f64| -> f64 {
        let point = golden_points()
            .into_iter()
            .min_by(|a, b| {
                (a.frequency_hz - target)
                    .abs()
                    .partial_cmp(&(b.frequency_hz - target).abs())
                    .expect("sin NaN")
            })
            .expect("golden no vacío");
        point.db
    };
    let drop_model = at(25.1) - at(199.4);
    let drop_golden = golden_db(25.1) - golden_db(199.4);
    println!("caída en 25 Hz: modelo {drop_model:.2} dB, golden {drop_golden:.2} dB");
    assert!(
        (drop_model - drop_golden).abs() < 0.3,
        "caída en la esquina Lm: modelo {drop_model:.2} dB vs golden {drop_golden:.2} dB"
    );
}

/// Polaridad: una corriente diferencial positiva (placa 1) da secundario
/// negativo, consistente con la realimentación negativa del lazo.
#[test]
fn polarity_is_negative_for_positive_differential_current() {
    let config = model_config();
    let dt = 1.0 / (SAMPLE_RATE_HZ * OVERSAMPLING);
    let mut model = OutputTransformerModel::new(config, dt).expect("modelo del OT");
    let mut secondary = 0.0;
    for _ in 0..2_000 {
        let (drop, _) = model.differential_drop(0.01);
        secondary = model.advance(-drop, 0.01).secondary_v;
    }
    assert!(secondary < 0.0, "polaridad invertida: vsec = {secondary}");
}
