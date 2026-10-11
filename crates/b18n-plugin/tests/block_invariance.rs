//! El procesador es invariante al tamaño de bloque del host (1/7/64/257/1024).

mod common;

use b18n_plugin::processor::B18nProcessor;

const BLOCK_SIZES: [usize; 5] = [1, 7, 64, 257, 1024];

fn process_in_blocks(input: &[f32], block: usize, oversampling: u8) -> Vec<f32> {
    let circuit = common::circuit();
    let spec = common::spec(48_000.0, input.len().max(1));
    let controls = common::controls();
    let mut processor = B18nProcessor::new(&circuit, spec).expect("procesador");
    let mut output = Vec::with_capacity(input.len());
    for chunk in input.chunks(block) {
        let mut block_buffer = chunk.to_vec();
        processor.process(&mut [&mut block_buffer], controls, oversampling, false);
        output.extend_from_slice(&block_buffer);
    }
    output
}

#[test]
fn output_is_block_size_invariant() {
    let input = common::sine(4096, 0.05, 0.3);
    let whole = process_in_blocks(&input, input.len(), 4);
    for block in BLOCK_SIZES {
        let chunked = process_in_blocks(&input, block, 4);
        assert_eq!(chunked, whole, "bloque de {block} frames");
    }
}

#[test]
fn empty_and_single_frame_blocks_are_safe() {
    let circuit = common::circuit();
    let spec = common::spec(48_000.0, 16);
    let controls = common::controls();
    let mut processor = B18nProcessor::new(&circuit, spec).expect("procesador");

    processor.process(&mut [], controls, 4, false);

    let mut single = [0.75_f32];
    processor.process(&mut [&mut single], controls, 4, false);
    assert!(single[0].is_finite());
}
