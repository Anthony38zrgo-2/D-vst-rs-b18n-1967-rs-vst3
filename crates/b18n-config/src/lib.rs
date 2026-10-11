//! Carga, validación y traducción del JSON histórico del B-18N 1967.
//!
//! El JSON se embebe en el binario final: el plugin no depende de una ruta de
//! disco en el equipo del usuario. La carga y la validación se realizan al
//! inicializar (nunca desde el callback de audio).

mod calibration;
mod errata;
mod mapping;
mod schema;
mod validation;

use std::fmt;

pub use b18n_dsp::circuit::{CircuitDefinition, FidelityLevel, MissingValue};
pub use calibration::{CalibrationEntry, CalibrationFile, CalibrationOrigin};
pub use errata::{sha256_hex, ErrataCorrection, HistoricalErrata};
pub use mapping::convert_to_circuit;
pub use schema::{HistoricalB18nJson, PotLawJson};
pub use validation::{uncharacterized_fields, validate_source, EXPECTED_SCHEMATIC_REVISION};

/// JSON canónico embebido (copiado sin modificar desde la fuente del proyecto).
pub const EMBEDDED_REFERENCE_JSON: &str =
    include_str!("../../../assets/models/B18N_1967_simulation_parameters.json");

/// SHA-256 del JSON canónico embebido.
///
/// Guarda de inmutabilidad: cualquier corrección documentada del circuito se
/// aplica desde `assets/models/b18n_1967_historical_errata.json` y **nunca**
/// editando este archivo.
pub const EXPECTED_REFERENCE_SHA256: &str =
    "d61d122d232a896b4269dd8769d54f6178540ed3b5afd655c9b403966d2a6a0b";

/// Overlay de calibración embebido (parámetros de modelo no históricos,
/// etiquetados como `estimated`/`measured`/`datasheet`).
pub const EMBEDDED_CALIBRATION_JSON: &str =
    include_str!("../../../assets/models/b18n_1967_calibration.json");

/// Errata histórica documentada embebida (correcciones con evidencia primaria).
/// Se aplica **antes** de construir `CircuitDefinition`; el JSON canónico no
/// se edita (véase [`EXPECTED_REFERENCE_SHA256`]).
pub const EMBEDDED_ERRATA_JSON: &str =
    include_str!("../../../assets/models/b18n_1967_historical_errata.json");

/// Errores de configuración tipados.
#[derive(Debug)]
pub enum ConfigError {
    /// El documento no es JSON válido o no coincide con el esquema esperado.
    Json(serde_json::Error),
    /// La revisión de esquema no es la exigida por este proyecto.
    UnexpectedRevision {
        expected: &'static str,
        found: String,
    },
    /// Un valor del documento carece de coherencia o unidades.
    InvalidValue {
        field: &'static str,
        reason: &'static str,
    },
    /// El overlay de calibración no cumple sus reglas.
    Calibration { message: String },
    /// La errata histórica no cumple sus reglas o no enlaza con el canónico.
    Errata { message: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(source) => write!(f, "JSON histórico inválido: {source}"),
            Self::UnexpectedRevision { expected, found } => write!(
                f,
                "revisión de esquema inesperada: se esperaba '{expected}', se encontró '{found}'"
            ),
            Self::InvalidValue { field, reason } => {
                write!(f, "valor inválido en '{field}': {reason}")
            }
            Self::Calibration { message } => write!(f, "calibración inválida: {message}"),
            Self::Errata { message } => write!(f, "errata inválida: {message}"),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(source) => Some(source),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(source: serde_json::Error) -> Self {
        Self::Json(source)
    }
}

/// Parsea y valida un documento histórico cualquiera.
///
/// No aplica la errata: es la vía cruda para inspeccionar el canónico tal cual.
pub fn parse_reference(json: &str) -> Result<HistoricalB18nJson, ConfigError> {
    let raw: HistoricalB18nJson = serde_json::from_str(json)?;
    validate_source(&raw)?;
    Ok(raw)
}

/// Aplica la errata histórica embebida al texto del JSON canónico y devuelve
/// el JSON efectivo.
///
/// La errata enlaza con el SHA-256 del canónico: si el texto no coincide, la
/// carga falla en lugar de aplicar correcciones a un modelo distinto.
pub fn apply_embedded_errata(json: &str) -> Result<String, ConfigError> {
    let errata = HistoricalErrata::parse(EMBEDDED_ERRATA_JSON)?;
    errata.apply_to_json_str(json)
}

/// Aplica el overlay de calibración embebido a un circuito ya convertido.
///
/// Imprescindible para el motor de válvulas: los modelos Koren no están en el
/// JSON canónico y no se inventan en el DSP.
pub fn apply_embedded_calibration(circuit: &mut CircuitDefinition) -> Result<(), ConfigError> {
    let calibration = CalibrationFile::parse(EMBEDDED_CALIBRATION_JSON)?;
    calibration.apply_to(circuit)
}

/// Carga el JSON embebido (con errata y calibración) y devuelve el circuito
/// tipado.
///
/// Orden: errata histórica -> validación -> `CircuitDefinition` -> overlay de
/// calibración. Debe llamarse al inicializar el motor/plugin, nunca dentro de
/// `process()`.
pub fn load_reference() -> Result<CircuitDefinition, ConfigError> {
    let effective = apply_embedded_errata(EMBEDDED_REFERENCE_JSON)?;
    let raw = parse_reference(&effective)?;
    let mut circuit = convert_to_circuit(&raw)?;
    apply_embedded_calibration(&mut circuit)?;
    Ok(circuit)
}
