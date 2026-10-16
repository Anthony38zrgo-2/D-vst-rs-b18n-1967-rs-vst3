//! B18-010/011: contraste del modelo 7027A contra el datasheet RCA 8-59.
//!
//! La referencia `power_7027a/7027a_rca_digitized.csv` la genera
//! `scripts/digitize_7027a_curves.py` a partir del escaneo 1-bit del datasheet
//! (p05: familia de placa a Vg1=0 con Ec2=50..450 V; p06: una curva de Ig2 sin
//! etiqueta, solo forma; p07: transferencia Ip/Ig2 a Vp=300 V, Ec2=300 V).
//! Los parámetros Koren vienen del overlay de calibración (`load_reference`).
//!
//! Las tolerancias son criterios de ingeniería del ajuste documentado en
//! `docs/MODEL_ASSUMPTIONS.md` (digitalización ±3 mA/±5 V en p05, ±2 mA/±1 V
//! en p07): no son garantías del fabricante.

use b18n_dsp::stages::output_transformer::OutputTransformerConfig;
use b18n_dsp::stages::power_7027a::{PowerStageConfig, PushPullStage};
use b18n_dsp::stages::speaker_impedance::ElectricalLoadConfig;
use b18n_dsp::stages::tube_models::KorenPentodeModel;

const DIGITIZED_CSV: &str =
    include_str!("../../../assets/fixtures/reference/power_7027a/7027a_rca_digitized.csv");

struct Point {
    figure: String,
    vg1_v: f64,
    ec2_v: f64,
    vp_v: f64,
    ip_ma: Option<f64>,
    ig2_ma: Option<f64>,
}

fn load_points() -> Vec<Point> {
    let mut points = Vec::new();
    for line in DIGITIZED_CSV.lines().skip(1) {
        let mut fields = line.split(',');
        let parse_optional = |field: Option<&str>| -> Option<f64> {
            field.expect("campo presente").trim().parse().ok()
        };
        let figure = fields.next().expect("figura").trim().to_owned();
        let vg1_v = parse_optional(fields.next()).unwrap_or(0.0);
        let ec2_v = fields
            .next()
            .expect("ec2")
            .trim()
            .parse()
            .expect("ec2 numérico");
        let vp_v = fields
            .next()
            .expect("vp")
            .trim()
            .parse()
            .expect("vp numérico");
        let ip_ma = parse_optional(fields.next());
        let ig2_ma = parse_optional(fields.next());
        points.push(Point {
            figure,
            vg1_v,
            ec2_v,
            vp_v,
            ip_ma,
            ig2_ma,
        });
    }
    assert!(points.len() > 200, "CSV incompleto: {}", points.len());
    points
}

fn model() -> KorenPentodeModel {
    let circuit = b18n_config::load_reference().expect("JSON + calibración");
    KorenPentodeModel::new(
        circuit
            .tube_models
            .koren_7027a
            .expect("modelo 7027A en calibración"),
    )
}

fn error_stats(errors: &[(f64, f64, f64, f64)]) -> (f64, f64, f64) {
    let mut relative: Vec<f64> = errors
        .iter()
        .map(|&(_, model, target, _)| (model - target).abs() / target)
        .collect();
    relative.sort_by(|a, b| a.partial_cmp(b).expect("sin NaN"));
    let median = relative[relative.len() / 2];
    let worst = relative[relative.len() - 1];
    let worst_point = errors
        .iter()
        .max_by(|a, b| {
            let da = (a.1 - a.2).abs() / a.2;
            let db = (b.1 - b.2).abs() / b.2;
            da.partial_cmp(&db).expect("sin NaN")
        })
        .expect("no vacío");
    (median, worst, worst_point.0)
}

/// p05: familia de placa Ip(Vp) a Vg1 = 0 con Ec2 = 50..450 V.
#[test]
fn plate_family_matches_rca_scan() {
    let model = model();
    let points = load_points();
    let mut errors: Vec<(f64, f64, f64, f64)> = Vec::new();
    for point in &points {
        if point.figure != "p05" || point.ip_ma.is_none() {
            continue;
        }
        let target = point.ip_ma.expect("p05 Ip") * 1e-3;
        let evaluated = model.plate_current(point.vp_v, point.ec2_v, point.vg1_v);
        errors.push((point.ec2_v, evaluated, target, point.vp_v));
    }
    assert_eq!(errors.len(), 207, "p05 incompleta");

    let (median, worst, worst_point) = error_stats(&errors);
    println!(
        "p05 Ip: n={} mediana {:.1}% peor {:.1}% (Ec2={worst_point:.0} V)",
        errors.len(),
        median * 100.0,
        worst * 100.0
    );
    assert!(median < 0.08, "mediana p05 {median}");
    assert!(worst < 0.25, "peor punto p05 {worst}");
}

/// p07: transferencia Ip(Vg1) a Vp = 300 V, Ec2 = 300 V.
#[test]
fn plate_transfer_matches_rca_scan() {
    let model = model();
    let points = load_points();
    let mut errors: Vec<(f64, f64, f64, f64)> = Vec::new();
    for point in &points {
        if point.figure != "p07" || point.ip_ma.is_none() {
            continue;
        }
        let target = point.ip_ma.expect("p07 Ip") * 1e-3;
        let evaluated = model.plate_current(point.vp_v, point.ec2_v, point.vg1_v);
        errors.push((point.vg1_v.abs(), evaluated, target, point.vp_v));
    }
    assert!(errors.len() >= 30, "p07 Ip incompleta: {}", errors.len());

    let (median, worst, worst_point) = error_stats(&errors);
    println!(
        "p07 Ip: n={} mediana {:.1}% peor {:.1}% (Vg1=-{worst_point:.0} V)",
        errors.len(),
        median * 100.0,
        worst * 100.0
    );
    assert!(median < 0.12, "mediana p07 Ip {median}");
    assert!(worst < 0.25, "peor punto p07 Ip {worst}");
}

/// p07: transferencia Ig2(Vg1) a Vp = 300 V (nivel y forma de la división).
#[test]
fn screen_transfer_matches_rca_scan() {
    let model = model();
    let points = load_points();
    let mut errors: Vec<(f64, f64, f64, f64)> = Vec::new();
    for point in &points {
        if point.figure != "p07" || point.ig2_ma.is_none() {
            continue;
        }
        let target = point.ig2_ma.expect("p07 Ig2") * 1e-3;
        let evaluated = model.screen_current(point.vp_v, point.ec2_v, point.vg1_v);
        errors.push((point.vg1_v.abs(), evaluated, target, point.vp_v));
    }
    assert!(errors.len() >= 8, "p07 Ig2 incompleta: {}", errors.len());

    let (median, worst, _) = error_stats(&errors);
    println!(
        "p07 Ig2: n={} mediana {:.1}% peor {:.1}%",
        errors.len(),
        median * 100.0,
        worst * 100.0
    );
    assert!(median < 0.20, "mediana p07 Ig2 {median}");
    assert!(worst < 0.35, "peor punto p07 Ig2 {worst}");
}

/// p06: forma Ig2(Vp) de la curva sin etiqueta (Ec2 = 300 V), normalizada a la
/// meseta (Vp >= 330 V).
#[test]
fn screen_knee_shape_matches_rca_scan() {
    let model = model();
    let points = load_points();

    let mut samples: Vec<(f64, f64)> = Vec::new();
    for point in &points {
        if point.figure == "p06" {
            if let Some(ig2_ma) = point.ig2_ma {
                samples.push((point.vp_v, ig2_ma * 1e-3));
            }
        }
    }
    assert!(samples.len() >= 40, "p06 incompleta: {}", samples.len());

    let plateau: Vec<f64> = samples
        .iter()
        .filter(|&&(vp, _)| vp >= 330.0)
        .map(|&(_, ig2)| ig2)
        .collect();
    assert!(!plateau.is_empty());
    let reference = plateau.iter().sum::<f64>() / plateau.len() as f64;

    let mut errors: Vec<(f64, f64, f64, f64)> = Vec::new();
    for &(vp, ig2) in &samples {
        let model_reference =
            model.screen_current(345.0, 300.0, 0.0) / model.plate_current(345.0, 300.0, 0.0);
        let model_ratio = model.screen_current(vp, 300.0, 0.0)
            / model.plate_current(vp, 300.0, 0.0)
            / model_reference;
        let measured_ratio = ig2 / reference;
        errors.push((vp, model_ratio, measured_ratio, 0.0));
    }
    let (median, worst, worst_point) = error_stats(&errors);
    println!(
        "p06 forma Ig2: n={} mediana {:.1}% peor {:.1}% (Vp={worst_point:.0} V)",
        errors.len(),
        median * 100.0,
        worst * 100.0
    );
    // La forma exponencial calibrada captura la subida en la rodilla; la
    // joroba fina por debajo de ~70 V queda documentada como limitación.
    assert!(median < 0.20, "mediana p06 forma {median}");
    assert!(worst < 0.45, "peor punto p06 forma {worst}");
}

/// Anclas tabuladas del datasheet (números impresos, no digitalizados).
#[test]
fn tabulated_anchors_match_rca_tables() {
    let model = model();

    // Clase A1: Va=250 V, Vg2=250 V, Vg1=-14 V -> Ip=72 mA, Ig2=5 mA,
    // gm=6000 umhos, rp=22.5 kOhm.
    let ip = model.plate_current(250.0, 250.0, -14.0);
    let ig2 = model.screen_current(250.0, 250.0, -14.0);
    let gm = model.transconductance(250.0, 250.0, -14.0);
    let rp = model.plate_resistance(250.0, 250.0, -14.0);
    println!(
        "clase A1: Ip {:.2}/{:.2} mA, Ig2 {:.2}/{:.2} mA, gm {:.0}/{:.0} umhos, rp {:.2}/{:.2} kOhm",
        ip * 1e3,
        72.0,
        ig2 * 1e3,
        5.0,
        gm * 1e6,
        6000.0,
        rp / 1e3,
        22.5
    );
    assert!((ip - 72e-3).abs() / 72e-3 < 0.15, "Ip clase A1 {ip}");
    assert!((ig2 - 5e-3).abs() / 5e-3 < 0.25, "Ig2 clase A1 {ig2}");
    assert!((gm - 6e-3).abs() / 6e-3 < 0.20, "gm clase A1 {gm}");
    assert!((rp - 22.5e3).abs() / 22.5e3 < 0.35, "rp clase A1 {rp}");

    // Bias fija (por válvula): (Va, Vg2, Vg1) -> Ip reposo.
    for (va, vs, vg, target_ma) in [
        (400.0, 300.0, -25.0, 51.0),
        (450.0, 350.0, -30.0, 47.5),
        (540.0, 400.0, -38.0, 50.0),
    ] {
        let evaluated = model.plate_current(va, vs, vg);
        let error = (evaluated * 1e3 - target_ma).abs() / target_ma;
        println!(
            "bias fija ({va}/{vs}/{vg}): Ip {:.2} mA (objetivo {target_ma}) err {:.1}%",
            evaluated * 1e3,
            error * 100.0
        );
        assert!(error < 0.35, "Ip bias fija {va} V: {error}");
    }
}

/// Reposo del par en el punto del B-18N: conduce y no supera la disipación
/// máxima de la ficha (35 W por placa).
#[test]
fn b18n_operating_point_conducts_within_rca_limits() {
    let circuit = b18n_config::load_reference().expect("JSON + calibración");
    let model = KorenPentodeModel::new(
        circuit
            .tube_models
            .koren_7027a
            .expect("modelo 7027A en calibración"),
    );
    let mut stage = PushPullStage::new(
        model,
        PowerStageConfig {
            supply_v: circuit.power_supply.bplus_idle_vdc,
            screen_supply_v: circuit.power_stage.screen_voltage_v_nominal,
            bias_v: circuit.power_stage.control_grid_voltage_v_nominal,
            transformer: OutputTransformerConfig {
                primary_plate_to_plate_ohms_nominal: circuit
                    .output_transformer
                    .primary_plate_to_plate_ohms_nominal,
                turns_ratio_8ohm: circuit.output_transformer.turns_ratio_primary_to_8ohm,
                turns_ratio_16ohm: circuit.output_transformer.turns_ratio_primary_to_16ohm,
                dc_primary_winding_ohms: circuit.output_transformer.dc_primary_winding_ohms,
                dc_secondary_8ohm_winding_ohms: circuit
                    .output_transformer
                    .dc_secondary_8ohm_winding_ohms,
                load: ElectricalLoadConfig::Resistive {
                    ohms: circuit.speaker.nominal_load_ohms,
                },
                magnetizing_inductance_h: circuit.output_transformer.winding_inductance_h,
                leakage_inductance_h: circuit.output_transformer.leakage_inductance_h,
                parasitic_capacitance_f: circuit.output_transformer.parasitic_capacitance_f,
                core_loss_ohms: circuit.output_transformer.core_loss_ohms,
                magnetizing_current_limit_a: circuit.output_transformer.magnetizing_current_limit_a,
            },
            grid_stopper_ohms: circuit.power_stage.grid_stopper_ohms_each,
            grid_return_ohms: circuit.power_stage.grid_return_ohms_each,
            grid_source_upper_ohms: 47_000.0,
            grid_source_lower_ohms: 47_000.0,
            coupling_farads: circuit.power_stage.inverter_to_grid_coupling_farads_each,
            sample_rate_hz: 48_000.0,
        },
    )
    .expect("modelo del OT-471");
    assert!(stage.solve_operating_point(), "reposo sin converger");
    let idle = stage.idle_plate_current_a();
    let dissipation = stage.plate_voltage(0) * idle;
    println!(
        "reposo B-18N: Ip {:.1} mA/tubo, Pd {:.1} W, Ig2 {:.2} mA",
        idle * 1e3,
        dissipation,
        stage.idle_screen_current_a() * 1e3
    );
    assert!(
        (0.015..0.070).contains(&idle),
        "reposo fuera del rango plausible: {idle} A"
    );
    assert!(
        dissipation < 35.0,
        "disipación de placa sobre el máximo RCA: {dissipation} W"
    );
    assert_eq!(stage.convergence_failures(), 0);
}
