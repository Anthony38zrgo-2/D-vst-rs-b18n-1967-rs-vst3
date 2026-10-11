//! Latencia reportada verificada con impulso (47 muestras a 4×, 0 a 1×).
//!
//! La corrección 93 → 47 se midió en P1.7: el cálculo anterior contaba cada
//! etapa de media banda al doble de su retardo real. Evidencia en
//! `oversampling.rs`: pico del impulso en 47, correlación cruzada de bursts en
//! 47 y centro de masas 46,28 (teórico 46,5). La cadena completa de la
//! instancia stock (controles por defecto, sin drive extra) pica en 46,
//! dentro de ±1 de lo reportado.
//!
//! Ola 8: el ácido del impulso con drive moderno no es medida de latencia —
//! el transitorio no lineal resuelto por el solver (sin freezes) desplaza el
//! ápice de la respuesta (46 → 49 medidos con `common::controls`, +6 dB in).
//! La latencia FIR no cambia y por eso se exige solo que el adaptador no
//! añada retardo respecto del motor directo.

mod common;

use b18n_dsp::{B18nEngine, ProcessSpec, RuntimeControls};
use b18n_plugin::processor::B18nProcessor;

fn impulse_adapter_response(oversampling: u8, controls: RuntimeControls) -> Vec<f32> {
    let circuit = common::circuit();
    let spec = common::spec(48_000.0, 4096);
    let mut processor = B18nProcessor::new(&circuit, spec).expect("procesador");

    let mut impulse = vec![0.0_f32; 4096];
    impulse[0] = 1.0;
    processor.process(&mut [&mut impulse], controls, oversampling, false);
    assert!(impulse.iter().all(|sample| sample.is_finite()));
    impulse
}

/// Respuesta del motor directo (referencia independiente del adaptador).
fn impulse_direct_response(oversampling: u8, controls: RuntimeControls) -> Vec<f32> {
    let circuit = common::circuit();
    let spec = ProcessSpec {
        oversampling,
        ..common::spec(48_000.0, 4096)
    };
    let mut engine = B18nEngine::new(circuit, spec).expect("motor directo");
    engine.set_controls(controls);

    let mut impulse = vec![0.0_f32; 4096];
    impulse[0] = 1.0;
    let mut output = vec![0.0_f32; 4096];
    engine.process_mono(&impulse, &mut output);
    output
}

fn peak_index(samples: &[f32]) -> usize {
    samples
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            a.abs()
                .partial_cmp(&b.abs())
                .expect("salida finita sin NaN")
        })
        .map(|(index, _)| index)
        .expect("respuesta no vacía")
}

#[test]
fn reported_latency_matches_the_impulse_peak() {
    let circuit = common::circuit();
    let spec = common::spec(48_000.0, 4096);
    let processor = B18nProcessor::new(&circuit, spec).expect("procesador");
    assert_eq!(processor.latency_samples(1), 0);
    assert_eq!(processor.latency_samples(4), 47);

    // Instancia stock (mandos por defecto): el pico de la respuesta
    // intrínseca cae en 46, dentro de ±1 de la latencia reportada.
    let stock_peak = peak_index(&impulse_adapter_response(4, RuntimeControls::default()));
    let reported = processor.latency_samples(4) as usize;
    assert!(
        stock_peak.abs_diff(reported) <= 1,
        "pico del impulso stock {stock_peak} vs latencia reportada {reported}"
    );

    // El adaptador no añade retardo respecto del motor directo, con mandos
    // por defecto y con el drive moderno (`common::controls`).
    let controls = common::controls();
    for oversampling in [1, 4] {
        assert_eq!(
            peak_index(&impulse_adapter_response(oversampling, controls)),
            peak_index(&impulse_direct_response(oversampling, controls)),
            "{oversampling}× sin retardo añadido"
        );
    }
}

#[test]
fn latency_does_not_depend_on_sample_rate() {
    let circuit = common::circuit();
    for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
        let spec = common::spec(rate, 512);
        let processor = B18nProcessor::new(&circuit, spec).expect("procesador");
        assert_eq!(processor.latency_samples(4), 47, "tasa {rate} Hz");
    }
}
