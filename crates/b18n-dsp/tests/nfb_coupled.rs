//! Independent SPICE comparison of B18-009 with frozen supply rails.
use b18n_dsp::stages::{
    output_network::OutputNetwork,
    output_transformer::OutputTransformerConfig,
    phase_splitter::PhaseInverter7199,
    power_7027a::{PowerStageConfig, PushPullStage},
    speaker_impedance::ElectricalLoadConfig,
    tube_models::KorenPentodeModel,
};
use std::f64::consts::PI;

fn network(feedback: bool, reactive: bool, rate: f64) -> OutputNetwork {
    let mut c = b18n_config::load_reference().expect("reference");
    c.output_transformer.negative_feedback.present = feedback;
    c.tube_models.preamp_supply_v = Some(380.0);
    c.phase_inverter.triode_supply_v = Some(460.0);
    let inverter = PhaseInverter7199::from_circuit(&c, rate).expect("7199");
    let t = &c.output_transformer;
    let mut power = PushPullStage::new(
        KorenPentodeModel::new(c.tube_models.koren_7027a.expect("7027A")),
        PowerStageConfig {
            supply_v: 505.0,
            screen_supply_v: 500.0,
            bias_v: -55.0,
            transformer: OutputTransformerConfig {
                primary_plate_to_plate_ohms_nominal: t.primary_plate_to_plate_ohms_nominal,
                turns_ratio_8ohm: t.turns_ratio_primary_to_8ohm,
                turns_ratio_16ohm: t.turns_ratio_primary_to_16ohm,
                dc_primary_winding_ohms: t.dc_primary_winding_ohms,
                dc_secondary_8ohm_winding_ohms: t.dc_secondary_8ohm_winding_ohms,
                load: ElectricalLoadConfig::Resistive { ohms: 8.0 },
                magnetizing_inductance_h: t.winding_inductance_h,
                leakage_inductance_h: t.leakage_inductance_h,
                parasitic_capacitance_f: t.parasitic_capacitance_f,
                core_loss_ohms: t.core_loss_ohms,
                magnetizing_current_limit_a: t.magnetizing_current_limit_a,
            },
            grid_stopper_ohms: c.power_stage.grid_stopper_ohms_each,
            grid_return_ohms: c.power_stage.grid_return_ohms_each,
            grid_source_upper_ohms: inverter.output_impedance_upper_ohms(),
            grid_source_lower_ohms: inverter.output_impedance_lower_ohms(),
            coupling_farads: c.power_stage.inverter_to_grid_coupling_farads_each,
            sample_rate_hz: rate,
        },
    )
    .expect("OT");
    assert!(power.solve_operating_point());
    let mut output = OutputNetwork::new(&c, &inverter, &power).expect("coupled DC");
    if reactive {
        output.set_load(ElectricalLoadConfig::LumpedEquivalent(
            c.speaker.lumped_equivalent.expect("RLC"),
        ));
    }
    output
}

fn compare_response(reactive_closed_only: bool) {
    let mut worst_db = 0.0_f64;
    let mut worst_phase = 0.0_f64;
    for row in include_str!("../../../assets/fixtures/reference/nfb/coupled_ac.csv")
        .lines()
        .skip(1)
    {
        let fields: Vec<_> = row.split(',').collect();
        let reactive = fields[0] == "reactiva";
        let feedback = fields[1] == "1";
        if (reactive && feedback) != reactive_closed_only {
            continue;
        }
        let frequency: f64 = fields[2].parse().expect("f");
        let golden_db: f64 = fields[3].parse().expect("dB");
        let golden_phase: f64 = fields[4].parse().expect("phase");
        let rate = if reactive && !feedback {
            768_000.0
        } else {
            192_000.0
        };
        let mut output = network(feedback, reactive, rate);
        let frames = if reactive && !feedback {
            262_144
        } else {
            65_536
        };
        let amplitude = 1e-4;
        let (mut re, mut im) = (0.0, 0.0);
        for index in 0..frames {
            let phase = 2.0 * PI * frequency * index as f64 / rate;
            let sample = output.process(amplitude * phase.cos());
            if index >= frames / 2 {
                re += sample * phase.cos();
                im -= sample * phase.sin();
            }
        }
        let gain = re.hypot(im) * 4.0 / (amplitude * frames as f64);
        let db = 20.0 * gain.log10();
        let phase = im.atan2(re).to_degrees();
        let error_db = (db - golden_db).abs();
        let error_phase = ((phase - golden_phase + 180.0).rem_euclid(360.0) - 180.0).abs();
        println!("reactive={reactive} feedback={feedback} {frequency} Hz: {db:.3} dB / {phase:.3} deg; errors {error_db:.3} dB / {error_phase:.3} deg; failures {}",output.convergence_failures());
        worst_db = worst_db.max(error_db);
        worst_phase = worst_phase.max(error_phase);
        assert_eq!(output.convergence_failures(), 0);
        // Engineering acceptance limits, independent of hardware fidelity.
        assert!(error_db < 0.5, "{row}: measured {db}");
        assert!(error_phase < 10.0, "{row}: measured {phase}");
    }
    println!("Worst independent AC errors: {worst_db:.4} dB / {worst_phase:.4} deg");
}

#[test]
fn open_and_nominal_closed_response_match_spice() {
    compare_response(false);
}

#[test]
#[ignore = "B18-009 candidate blocked: reactive closed-loop divergence; docs/B18_009_STATUS.md"]
fn reactive_closed_response_pending_b18_009() {
    compare_response(true);
}

#[test]
fn dc_operating_points_match_independent_spice() {
    for row in include_str!("../../../assets/fixtures/reference/nfb/coupled_dc.csv")
        .lines()
        .skip(1)
    {
        let fields: Vec<_> = row.split(',').collect();
        let output = network(fields[1] == "1", fields[0] == "reactiva", 192_000.0);
        let nodes = output.node_voltages();
        for (index, field) in fields[2..].iter().enumerate() {
            let reference: f64 = field.parse().expect("DC voltage");
            assert!(
                (nodes[index] - reference).abs() < 0.05,
                "{row}: node {index} = {}",
                nodes[index]
            );
        }
    }
}
