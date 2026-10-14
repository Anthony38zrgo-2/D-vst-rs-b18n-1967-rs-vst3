//! El callback de audio del adaptador no asigna memoria.
//!
//! Un allocador contador envuelve al del sistema; durante la ventana marcada
//! cualquier `alloc`/`realloc`/`alloc_zeroed` incrementa el contador. El test
//! se mantiene solo en este binario para que ningún otro test del proceso
//! interfiera con la ventana.

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use b18n_plugin::processor::B18nProcessor;

static FORBID_ALLOC: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if FORBID_ALLOC.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if FORBID_ALLOC.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if FORBID_ALLOC.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[test]
fn process_does_not_allocate() {
    let circuit = common::circuit();
    let spec = common::spec(48_000.0, 1024);
    let controls = common::controls();
    let mut processor = B18nProcessor::new(&circuit, spec).expect("procesador");

    let mut left = vec![0.0_f32; 1024];
    let mut right = vec![0.0_f32; 1024];
    // La lista de canales se prepara fuera de la ventana (no es del callback).
    let mut slices: Vec<&mut [f32]> = vec![&mut left, &mut right];

    // Calentamiento: cubre cualquier inicialización perezosa de las etapas.
    processor.process(&mut slices, controls, 4, false);

    ALLOCATIONS.store(0, Ordering::SeqCst);
    FORBID_ALLOC.store(true, Ordering::SeqCst);

    processor.process(&mut slices, controls, 4, false);
    processor.process(&mut slices, controls, 1, false);
    let mut speaker_controls = controls;
    speaker_controls.speaker_profile = b18n_dsp::SpeakerProfile::Eighteen;
    processor.process(&mut slices, speaker_controls, 2, false);
    speaker_controls.speaker_profile = b18n_dsp::SpeakerProfile::Fifteen;
    processor.process(&mut slices, speaker_controls, 2, true);
    processor.process(&mut slices, controls, 4, true);
    processor.reset();

    FORBID_ALLOC.store(false, Ordering::SeqCst);

    assert_eq!(
        ALLOCATIONS.load(Ordering::SeqCst),
        0,
        "asignaciones detectadas en process/reset"
    );
}
