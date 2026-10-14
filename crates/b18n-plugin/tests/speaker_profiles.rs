mod common;
use b18n_dsp::{B18nEngine, ProcessSpec, SpeakerProfile};
use b18n_plugin::processor::B18nProcessor;
#[test]
fn speaker_modes_match_direct_engine_and_are_block_invariant_at_all_factors() {
    let circuit = common::circuit();
    let input = common::sine(2048, 0.15, 0.3);
    for factor in [1, 2, 4] {
        for profile in [
            SpeakerProfile::Direct,
            SpeakerProfile::Eighteen,
            SpeakerProfile::Fifteen,
        ] {
            let spec = ProcessSpec {
                oversampling: factor,
                ..common::spec(48000.0, 2048)
            };
            let mut controls = common::controls();
            controls.speaker_profile = profile;
            let mut engine = B18nEngine::new(circuit.clone(), spec).unwrap();
            engine.set_controls(controls);
            let mut expected = vec![0.0; input.len()];
            engine.process_mono(&input, &mut expected);
            for block in [1, 257, 2048] {
                let mut processor = B18nProcessor::new(&circuit, spec).unwrap();
                let mut actual = input.clone();
                for chunk in actual.chunks_mut(block) {
                    processor.process(&mut [chunk], controls, factor, false);
                }
                assert_eq!(actual, expected);
                processor.reset();
                actual.copy_from_slice(&input);
                for chunk in actual.chunks_mut(block) {
                    processor.process(&mut [chunk], controls, factor, false);
                }
                assert_eq!(actual, expected);
            }
        }
    }
}

#[test]
fn two_times_is_stable_under_drive_across_preferred_rates() {
    for rate in ProcessSpec::PREFERRED_SAMPLE_RATES_HZ {
        for amplitude in [0.25, 1.0, 4.0] {
            let spec = ProcessSpec {
                sample_rate_hz: rate,
                max_block_frames: 8192,
                oversampling: 2,
            };
            let mut engine = B18nEngine::new(common::circuit(), spec).unwrap();
            engine.set_controls(common::controls());
            for i in 0..8192 {
                let input =
                    (2.0 * std::f64::consts::PI * 997.0 * i as f64 / rate).sin() as f32 * amplitude;
                let output = engine.process_sample(input);
                assert!(output.is_finite() && output.abs() < 500.0);
            }
            assert!(engine.convergence_failures() < 8192);
        }
    }
}
