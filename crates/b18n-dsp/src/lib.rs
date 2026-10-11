//! Núcleo DSP del B-18N/B-15ND, revisión de esquema `1967-05`.
//!
//! Este crate no conoce NIH-plug, VST3, el sistema de archivos ni `serde`: solo
//! tipos físicos del circuito, controles de usuario y el motor de procesamiento.
//! La traducción desde el JSON histórico la realiza `b18n-config`.
//!
//! Regla del proyecto: ningún valor eléctrico se inventa aquí. Los campos sin
//! caracterizar llegan como `None` y se listan en el campo `missing` de
//! [`CircuitDefinition`].

pub mod audio_context;
pub mod circuit;
pub mod controls;
pub mod engine;
pub mod numerics;
pub mod stages;
pub mod state;
pub mod utilities;

pub use audio_context::ProcessSpec;
pub use circuit::{
    CircuitDefinition, FidelityLevel, MissingValue, PotLaw, PotValues, TriodeStageValues,
};
pub use controls::{volume_fraction, InputChannel, RuntimeControls};
pub use engine::{B18nEngine, EngineError};
pub use stages::tone_stack::{ToneStackControls, ToneStackDesign, ToneStackError};
