//! Utilidades compartidas por los tests de integración del plugin.

#![allow(dead_code)]

use b18n_dsp::{CircuitDefinition, ProcessSpec, RuntimeControls};

/// Modelo canónico embebido (errata + calibración incluidas).
pub fn circuit() -> CircuitDefinition {
    b18n_config::load_reference().expect("modelo histórico cargable")
}

/// Especificación de proceso a 48 kHz con el tamaño de bloque pedido.
pub fn spec(sample_rate_hz: f64, max_block_frames: usize) -> ProcessSpec {
    ProcessSpec {
        sample_rate_hz,
        max_block_frames,
        oversampling: 4,
    }
}

/// Controles no triviales para que la paridad no se pruebe solo en defaults.
pub fn controls() -> RuntimeControls {
    RuntimeControls {
        input_trim_db: 6.0,
        volume_1: 0.7,
        bass_1: 0.3,
        treble_1: 0.8,
        volume_2: 0.6,
        bass_2: 0.4,
        treble_2: 0.2,
        output_trim_db: -3.0,
        ..RuntimeControls::default()
    }
}

/// Senoidal de prueba determinista.
pub fn sine(frames: usize, omega: f32, amplitude: f32) -> Vec<f32> {
    (0..frames)
        .map(|index| (index as f32 * omega).sin() * amplitude)
        .collect()
}
