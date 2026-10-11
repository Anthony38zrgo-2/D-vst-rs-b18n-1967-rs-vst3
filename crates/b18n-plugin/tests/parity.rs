//! El adaptador no altera el DSP: procesador vs motor directo, sample-exact.
//!
//! Referencia: `b18n-render` procesa cada canal con un `B18nEngine` directo;
//! si el adaptador reproduce esa salida bit a bit, el WAV del plugin coincide
//! con el del render con los mismos parámetros y estado inicial.

mod common;

use b18n_dsp::B18nEngine;
use b18n_plugin::processor::B18nProcessor;

#[test]
fn mono_processor_matches_direct_engine_at_4x() {
    let circuit = common::circuit();
    let spec = common::spec(48_000.0, 4096);
    let controls = common::controls();
    let input = common::sine(4096, 0.05, 0.3);

    let mut reference = B18nEngine::new(circuit.clone(), spec).expect("motor de referencia");
    reference.set_controls(controls);
    let mut expected = vec![0.0_f32; input.len()];
    reference.process_mono(&input, &mut expected);

    let mut processor = B18nProcessor::new(&circuit, spec).expect("procesador");
    let mut channel = input.clone();
    processor.process(&mut [&mut channel], controls, 4, false);

    assert_eq!(
        channel, expected,
        "adaptador y motor directo deben coincidir"
    );
}

#[test]
fn mono_processor_matches_direct_engine_at_1x() {
    let circuit = common::circuit();
    let spec = common::spec(44_100.0, 2048);
    let controls = common::controls();
    let input = common::sine(2048, 0.03, 0.25);

    let mut reference = B18nEngine::new(
        circuit.clone(),
        b18n_dsp::ProcessSpec {
            oversampling: 1,
            ..spec
        },
    )
    .expect("motor de referencia");
    reference.set_controls(controls);
    let mut expected = vec![0.0_f32; input.len()];
    reference.process_mono(&input, &mut expected);

    let mut processor = B18nProcessor::new(&circuit, spec).expect("procesador");
    let mut channel = input.clone();
    processor.process(&mut [&mut channel], controls, 1, false);

    assert_eq!(channel, expected);
}

#[test]
fn stereo_is_two_independent_mono_engines() {
    let circuit = common::circuit();
    let spec = common::spec(96_000.0, 2048);
    let controls = common::controls();
    let left = common::sine(2048, 0.05, 0.3);
    let right = common::sine(2048, 0.11, 0.2);

    let render = |input: &[f32]| {
        let mut engine = B18nEngine::new(circuit.clone(), spec).expect("motor");
        engine.set_controls(controls);
        let mut output = vec![0.0_f32; input.len()];
        engine.process_mono(input, &mut output);
        output
    };
    let expected_left = render(&left);
    let expected_right = render(&right);

    let mut processor = B18nProcessor::new(&circuit, spec).expect("procesador");
    let mut channel_left = left;
    let mut channel_right = right;
    processor.process(
        &mut [&mut channel_left, &mut channel_right],
        controls,
        4,
        false,
    );

    assert_eq!(channel_left, expected_left, "canal izquierdo");
    assert_eq!(channel_right, expected_right, "canal derecho");
    assert_ne!(
        channel_left, channel_right,
        "los canales no deben mezclarse"
    );
}
