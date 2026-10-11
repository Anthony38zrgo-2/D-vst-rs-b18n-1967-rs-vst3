//! Nivel de salida y robustez del solver a niveles DAW (Fase 1/Ola 8).
//!
//! Convención documentada (`MODEL_ASSUMPTIONS`): amplitud digital 1.0 = 1 V
//! de pico. Una instancia recién creada (mandos por defecto del plugin) no
//! debe rebasar la escala completa del host, y el solver no debe congelar
//! estado (fallos de convergencia = discontinuidades por muestra).

mod common;

use b18n_dsp::RuntimeControls;
use b18n_plugin::params::B18nParams;
use b18n_plugin::processor::B18nProcessor;

const RATE: f64 = 48_000.0;
const FRAMES: usize = 24_000;

fn full_scale_sine(frequency_hz: f64, amplitude: f32) -> Vec<f32> {
    (0..FRAMES)
        .map(|index| {
            (2.0 * std::f64::consts::PI * frequency_hz * index as f64 / RATE).sin() as f32
                * amplitude
        })
        .collect()
}

fn peak(samples: &[f32]) -> f32 {
    samples
        .iter()
        .fold(0.0_f32, |acc, value| acc.max(value.abs()))
}

#[test]
fn stock_instance_peaks_below_full_scale() {
    // Instancia nueva (sin ajuste del usuario): los trims por defecto deben
    // situar la salida dentro de la escala digital del host.
    let circuit = common::circuit();
    let spec = common::spec(RATE, 1024);
    let controls: RuntimeControls = B18nParams::default().runtime_controls();
    let mut processor = B18nProcessor::new(&circuit, spec).expect("procesador");

    let mut left = full_scale_sine(110.0, 1.0);
    let mut right = full_scale_sine(220.0, 1.0);
    processor.process(&mut [&mut left, &mut right], controls, 4, false);

    let stock_peak = peak(&left).max(peak(&right));
    println!("pico de la instancia por defecto: {stock_peak:.3}");
    assert!(
        stock_peak < 1.0,
        "la instancia por defecto rebasa la escala completa del host: pico {stock_peak:.3} \
         (el host/DAC recorta en digital y suena a crunch)"
    );
}

#[test]
fn no_solver_failures_at_daw_levels() {
    let circuit = common::circuit();
    let spec = common::spec(RATE, 1024);
    for amplitude in [0.2_f32, 1.0] {
        let mut processor = B18nProcessor::new(&circuit, spec).expect("procesador");
        let controls = RuntimeControls::default();
        let mut left = full_scale_sine(110.0, amplitude);
        let mut right = full_scale_sine(220.0, amplitude);
        processor.process(&mut [&mut left, &mut right], controls, 4, false);
        let failures = processor.convergence_failures(4);
        assert_eq!(
            failures, 0,
            "a {amplitude} V el solver congela estado en {failures} muestras \
             (discontinuidad por sample = crunch digital)"
        );
    }
}
