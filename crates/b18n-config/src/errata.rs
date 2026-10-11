//! Errata histórica documentada, separada del JSON canónico.
//!
//! El JSON canónico es inmutable (véase la guarda de SHA-256 en los tests).
//! Las correcciones con evidencia primaria —como el intercambio 270 kΩ/470 kΩ
//! de la 7199— se declaran aquí, se aplican sobre el árbol JSON **antes** de
//! construir `CircuitDefinition` y quedan registradas en la huella efectiva
//! del modelo. No son parámetros de calibración ni sobreescriben el original.

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{ConfigError, EXPECTED_SCHEMATIC_REVISION};

/// Documento de errata aplicable a una revisión concreta del JSON canónico.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoricalErrata {
    /// Revisión de esquema a la que aplica (debe coincidir con la exigida).
    pub target_revision: String,
    /// SHA-256 del JSON canónico al que se enlaza esta errata.
    pub base_sha256: String,
    /// Motivo documentado de la errata.
    pub reason: String,
    /// Correcciones de valores existentes; cada una exige evidencia y fecha.
    #[serde(default)]
    pub corrections: Vec<ErrataCorrection>,
    /// Adiciones de componentes trazados que el JSON inicial no listaba.
    #[serde(default)]
    pub additions: Vec<ErrataAddition>,
}

/// Componente trazado en el esquema que el JSON canónico no lista.
///
/// No es una corrección de valor: es un dato nuevo con evidencia primaria
/// (p. ej. la 150 kΩ de pantalla del pentodo 7199).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrataAddition {
    /// Ruta con puntos del nodo a crear (no puede existir en el canónico).
    pub path: String,
    /// Valor JSON que se inserta (un número, objeto o lista).
    pub value: Value,
    /// Evidencia reproducible (esquema, manual, coordenadas, contraste).
    pub evidence: String,
    /// Fecha de la adición (`YYYY-MM-DD`).
    pub date: String,
}

/// Corrección puntual de un valor del JSON canónico.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ErrataCorrection {
    /// Ruta con puntos dentro del objeto JSON (p. ej. `phase_inverter.x`).
    pub path: String,
    /// Valor erróneo actual del canónico; guarda contra la deriva del archivo.
    pub json_value: f64,
    /// Valor corregido según la fuente primaria.
    pub corrected_value: f64,
    /// Unidad del valor corregido.
    pub unit: String,
    /// Pin del zócalo asociado, si la corrección describe un componente.
    pub pin: u8,
    /// Electrodo o función del componente.
    pub electrode: String,
    /// Nodo/carril de alimentación del extremo opuesto al pin.
    pub from_node: String,
    /// Nodo al que conecta el pin.
    pub to_node: String,
    /// Evidencia reproducible (esquema, manual, coordenadas, contraste).
    pub evidence: String,
    /// Fecha de la corrección (`YYYY-MM-DD`).
    pub date: String,
}

impl HistoricalErrata {
    /// Parsea y valida un documento de errata.
    pub fn parse(json: &str) -> Result<Self, ConfigError> {
        let errata: Self = serde_json::from_str(json)?;
        errata.validate()?;
        Ok(errata)
    }

    /// Aplica la errata a un texto JSON canónico y devuelve el JSON efectivo.
    ///
    /// Verifica primero que el texto enlaza con [`Self::base_sha256`]; después
    /// exige que cada valor actual coincida con `json_value` (guarda anti-deriva)
    /// antes de sustituirlo.
    pub fn apply_to_json_str(&self, json: &str) -> Result<String, ConfigError> {
        let digest = sha256_hex(json.as_bytes());
        if digest != self.base_sha256.to_lowercase() {
            return Err(errata_error(format!(
                "el JSON no coincide con base_sha256: se esperaba '{}', se encontró '{digest}'",
                self.base_sha256
            )));
        }
        let mut root: Value = serde_json::from_str(json)?;
        self.apply_to_value(&mut root)?;
        serde_json::to_string(&root).map_err(ConfigError::from)
    }

    /// Aplica las correcciones sobre un árbol JSON ya parseado.
    pub fn apply_to_value(&self, root: &mut Value) -> Result<usize, ConfigError> {
        let mut applied = 0_usize;
        for correction in &self.corrections {
            let current = number_at(root, &correction.path).ok_or_else(|| {
                errata_error(format!(
                    "la ruta '{}' no existe o no es numérica en el JSON canónico",
                    correction.path
                ))
            })?;
            if current != correction.json_value {
                return Err(errata_error(format!(
                    "deriva detectada en '{}': el canónico vale {current}, la errata esperaba {}",
                    correction.path, correction.json_value
                )));
            }
            set_number_at(root, &correction.path, correction.corrected_value).ok_or_else(|| {
                errata_error(format!("no se pudo escribir la ruta '{}'", correction.path))
            })?;
            applied += 1;
        }
        for addition in &self.additions {
            if has_path(root, &addition.path) {
                return Err(errata_error(format!(
                    "la adición '{}' ya existe en el JSON canónico; sería una redefinición silenciosa",
                    addition.path
                )));
            }
            set_value_at(root, &addition.path, addition.value.clone()).ok_or_else(|| {
                errata_error(format!("no se pudo crear la ruta '{}'", addition.path))
            })?;
            applied += 1;
        }
        Ok(applied)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        if self.target_revision != EXPECTED_SCHEMATIC_REVISION {
            return Err(errata_error(format!(
                "target_revision '{}' distinta de '{}'",
                self.target_revision, EXPECTED_SCHEMATIC_REVISION
            )));
        }
        if self.base_sha256.len() != 64 || !self.base_sha256.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(errata_error(
                "base_sha256 debe ser un SHA-256 hexadecimal de 64 caracteres".to_owned(),
            ));
        }
        if self.reason.trim().is_empty() {
            return Err(errata_error("la errata no documenta un motivo".to_owned()));
        }
        if self.corrections.is_empty() && self.additions.is_empty() {
            return Err(errata_error(
                "la errata no contiene correcciones ni adiciones".to_owned(),
            ));
        }
        for correction in &self.corrections {
            validate_path(&correction.path)?;
            if correction.unit.trim().is_empty()
                || correction.electrode.trim().is_empty()
                || correction.from_node.trim().is_empty()
                || correction.to_node.trim().is_empty()
                || correction.evidence.trim().is_empty()
                || correction.date.trim().is_empty()
            {
                return Err(errata_error(format!(
                    "'{}' no documenta unidad, electrodo, nodos, evidencia y fecha",
                    correction.path
                )));
            }
            if !correction.corrected_value.is_finite() || correction.corrected_value <= 0.0 {
                return Err(errata_error(format!(
                    "'{}' debe corregir a un valor finito y positivo",
                    correction.path
                )));
            }
        }
        for addition in &self.additions {
            validate_path(&addition.path)?;
            if addition.value.is_null() {
                return Err(errata_error(format!(
                    "la adición '{}' no aporta valor",
                    addition.path
                )));
            }
            if addition.evidence.trim().is_empty() || addition.date.trim().is_empty() {
                return Err(errata_error(format!(
                    "'{}' no documenta evidencia y fecha",
                    addition.path
                )));
            }
        }
        Ok(())
    }
}

/// SHA-256 en hexadecimal minúsculas.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn validate_path(path: &str) -> Result<(), ConfigError> {
    let segments: Vec<&str> = path.split('.').collect();
    if segments.len() < 2 || segments.iter().any(|segment| segment.is_empty()) {
        return Err(errata_error(format!(
            "ruta inválida '{path}': se espera 'seccion.campo'"
        )));
    }
    if segments[0] == "identity" {
        return Err(errata_error(format!(
            "'{path}' intenta corregir la identidad del documento; no está permitido"
        )));
    }
    Ok(())
}

fn number_at(root: &Value, path: &str) -> Option<f64> {
    let mut current = root;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    current.as_f64()
}

fn has_path(root: &Value, path: &str) -> bool {
    let mut current = root;
    for segment in path.split('.') {
        match current.get(segment) {
            Some(next) => current = next,
            None => return false,
        }
    }
    true
}

fn set_number_at(root: &mut Value, path: &str, value: f64) -> Option<()> {
    set_value_at(root, path, Value::from(value))
}

fn set_value_at(root: &mut Value, path: &str, value: Value) -> Option<()> {
    let mut segments = path.split('.').peekable();
    let first = segments.next()?;
    let mut current = root.get_mut(first)?;
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            current.as_object_mut()?.insert(segment.to_owned(), value);
            return Some(());
        }
        current = current.get_mut(segment)?;
    }
    Some(())
}

fn errata_error(message: String) -> ConfigError {
    ConfigError::Errata { message }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANONICAL: &str = r#"{
        "identity": {"model": "B-18N Portaflex"},
        "phase_inverter": {"pentode_plate_resistor_ohms": 470000, "pentode_screen_feed_resistor_ohms": 270000}
    }"#;

    fn errata_replacing(path: &str) -> HistoricalErrata {
        let json = format!(
            r#"{{
                "target_revision": "1967-05",
                "base_sha256": "{}",
                "reason": "prueba",
                "corrections": [{{
                    "path": "{path}",
                    "json_value": 470000,
                    "corrected_value": 270000,
                    "unit": "ohm",
                    "pin": 2,
                    "electrode": "placa",
                    "from_node": "carril",
                    "to_node": "pin 2",
                    "evidence": "prueba",
                    "date": "2026-10-08"
                }}]
            }}"#,
            sha256_hex(CANONICAL.as_bytes())
        );
        HistoricalErrata::parse(&json).expect("errata de prueba válida")
    }

    #[test]
    fn applies_correction_and_keeps_base_hash_check() {
        let errata = errata_replacing("phase_inverter.pentode_plate_resistor_ohms");
        let effective = errata.apply_to_json_str(CANONICAL).expect("aplica");
        let value: Value = serde_json::from_str(&effective).expect("JSON válido");
        assert_eq!(
            number_at(&value, "phase_inverter.pentode_plate_resistor_ohms"),
            Some(270000.0)
        );
        assert_eq!(
            number_at(&value, "phase_inverter.pentode_screen_feed_resistor_ohms"),
            Some(270000.0)
        );
    }

    #[test]
    fn rejects_base_hash_mismatch() {
        let errata = errata_replacing("phase_inverter.pentode_plate_resistor_ohms");
        let tampered = CANONICAL.replace("Portaflex", "Portaflux");
        assert!(matches!(
            errata.apply_to_json_str(&tampered),
            Err(ConfigError::Errata { .. })
        ));
    }

    #[test]
    fn rejects_value_drift() {
        let errata = errata_replacing("phase_inverter.pentode_screen_feed_resistor_ohms");
        // json_value 470000 pero el canónico tiene 270000 en ese campo.
        assert!(matches!(
            errata.apply_to_json_str(CANONICAL),
            Err(ConfigError::Errata { .. })
        ));
    }

    #[test]
    fn rejects_identity_corrections() {
        let json = r#"{
            "target_revision": "1967-05",
            "base_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
            "reason": "prueba",
            "corrections": [{
                "path": "identity.model",
                "json_value": 1,
                "corrected_value": 2,
                "unit": "1",
                "pin": 1,
                "electrode": "x",
                "from_node": "x",
                "to_node": "x",
                "evidence": "x",
                "date": "2026-10-08"
            }]
        }"#;
        assert!(matches!(
            HistoricalErrata::parse(json),
            Err(ConfigError::Errata { .. })
        ));
    }

    #[test]
    fn rejects_unsupported_target_revision() {
        let json = r#"{
            "target_revision": "1966-01",
            "base_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
            "reason": "prueba",
            "corrections": [{
                "path": "phase_inverter.pentode_plate_resistor_ohms",
                "json_value": 470000,
                "corrected_value": 270000,
                "unit": "ohm",
                "pin": 2,
                "electrode": "placa",
                "from_node": "carril",
                "to_node": "pin 2",
                "evidence": "prueba",
                "date": "2026-10-08"
            }]
        }"#;
        assert!(matches!(
            HistoricalErrata::parse(json),
            Err(ConfigError::Errata { .. })
        ));
    }

    fn errata_with_addition(path: &str) -> HistoricalErrata {
        let json = format!(
            r#"{{
                "target_revision": "1967-05",
                "base_sha256": "{}",
                "reason": "prueba",
                "corrections": [],
                "additions": [{{
                    "path": "{path}",
                    "value": [{{"ohms": 150000}}],
                    "evidence": "prueba",
                    "date": "2026-10-08"
                }}]
            }}"#,
            sha256_hex(CANONICAL.as_bytes())
        );
        HistoricalErrata::parse(&json).expect("errata de prueba válida")
    }

    #[test]
    fn applies_addition_to_new_path() {
        let errata = errata_with_addition("phase_inverter.additional_traced_resistors");
        let effective = errata.apply_to_json_str(CANONICAL).expect("aplica");
        let value: Value = serde_json::from_str(&effective).expect("JSON válido");
        assert!(value["phase_inverter"]["additional_traced_resistors"].is_array());
    }

    #[test]
    fn rejects_addition_that_redefines_existing_path() {
        let errata = errata_with_addition("phase_inverter.pentode_plate_resistor_ohms");
        assert!(matches!(
            errata.apply_to_json_str(CANONICAL),
            Err(ConfigError::Errata { .. })
        ));
    }
}
