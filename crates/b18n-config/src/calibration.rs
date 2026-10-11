//! Overlay de calibración, separado del JSON canónico.
//!
//! Cualquier valor no respaldado por la documentación histórica se registra
//! aquí con unidad, origen, método y fecha. El overlay **no** puede modificar
//! los nominales protegidos (resistencias, condensadores, tensiones).

use std::collections::BTreeMap;

use b18n_dsp::circuit::{CircuitDefinition, KorenPentodeParameters, KorenTriodeParameters};
use serde::Deserialize;

use crate::ConfigError;

/// Prefijos del JSON canónico que el overlay no puede redefinir.
const PROTECTED_PREFIXES: [&str; 6] = [
    "identity",
    "controls",
    "preamp",
    "phase_inverter",
    "power_stage",
    "power_supply",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationFile {
    pub target_revision: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub assumptions: BTreeMap<String, CalibrationEntry>,
    #[serde(default)]
    pub measured: BTreeMap<String, CalibrationEntry>,
    #[serde(default)]
    pub provenance: Vec<String>,
}

/// Entrada de calibración con trazabilidad obligatoria.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationEntry {
    pub value: f64,
    pub unit: String,
    pub origin: CalibrationOrigin,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CalibrationOrigin {
    Estimated,
    Measured,
    Datasheet,
}

impl CalibrationFile {
    /// Parsea y valida un overlay de calibración.
    pub fn parse(json: &str) -> Result<Self, ConfigError> {
        let file: Self = serde_json::from_str(json)?;
        file.validate()
    }

    /// Aplica el overlay al circuito: solo parámetros de modelo, nunca
    /// nominales de componentes (protegidos por [`Self::validate`]).
    pub fn apply_to(&self, circuit: &mut CircuitDefinition) -> Result<(), ConfigError> {
        let value = |key: &str| -> Option<f64> {
            self.assumptions
                .get(key)
                .or_else(|| self.measured.get(key))
                .map(|entry| entry.value)
        };

        let required = [
            ("mu", value("tube_models.6sl7.koren.mu")),
            ("ex", value("tube_models.6sl7.koren.ex")),
            ("kg1", value("tube_models.6sl7.koren.kg1")),
            ("kp", value("tube_models.6sl7.koren.kp")),
            ("kvb", value("tube_models.6sl7.koren.kvb")),
        ];
        let present = required.iter().filter(|(_, value)| value.is_some()).count();
        circuit.tube_models.koren_6sl7 = match present {
            0 => None,
            5 => Some(KorenTriodeParameters {
                mu: required[0].1.expect("presente"),
                ex: required[1].1.expect("presente"),
                kg1: required[2].1.expect("presente"),
                kp: required[3].1.expect("presente"),
                kvb: required[4].1.expect("presente"),
                vct: value("tube_models.6sl7.koren.vct").unwrap_or(0.0),
                grid_conduction_threshold_v: value("tube_models.6sl7.grid.threshold_v")
                    .unwrap_or(-0.3),
                grid_conduction_resistance_ohms: value("tube_models.6sl7.grid.resistance_ohms")
                    .unwrap_or(1_000.0),
            }),
            _ => {
                return Err(ConfigError::Calibration {
                    message: "modelo Koren del 6SL7 incompleto: se exigen mu, ex, kg1, kp y kvb"
                        .to_owned(),
                })
            }
        };

        let required_pentode = [
            ("mu", value("tube_models.7027a.koren.mu")),
            ("ex", value("tube_models.7027a.koren.ex")),
            ("kg1", value("tube_models.7027a.koren.kg1")),
            ("kg2", value("tube_models.7027a.koren.kg2")),
            ("kp", value("tube_models.7027a.koren.kp")),
            ("kvb", value("tube_models.7027a.koren.kvb")),
        ];
        let present_pentode = required_pentode
            .iter()
            .filter(|(_, value)| value.is_some())
            .count();
        circuit.tube_models.koren_7027a = match present_pentode {
            0 => None,
            6 => Some(KorenPentodeParameters {
                mu: required_pentode[0].1.expect("presente"),
                ex: required_pentode[1].1.expect("presente"),
                kg1: required_pentode[2].1.expect("presente"),
                kg2: required_pentode[3].1.expect("presente"),
                kp: required_pentode[4].1.expect("presente"),
                kvb: required_pentode[5].1.expect("presente"),
                vct: value("tube_models.7027a.koren.vct").unwrap_or(0.0),
                grid_conduction_threshold_v: value("tube_models.7027a.grid.threshold_v")
                    .unwrap_or(0.0),
                grid_conduction_resistance_ohms: value("tube_models.7027a.grid.resistance_ohms")
                    .unwrap_or(500.0),
            }),
            _ => {
                return Err(ConfigError::Calibration {
                    message: "modelo Koren de la 7027A incompleto: se exigen mu, ex, kg1, kg2, kp \
                              y kvb"
                        .to_owned(),
                })
            }
        };

        let required_7199_pentode = [
            ("mu", value("tube_models.7199.pentode.koren.mu")),
            ("ex", value("tube_models.7199.pentode.koren.ex")),
            ("kg1", value("tube_models.7199.pentode.koren.kg1")),
            ("kg2", value("tube_models.7199.pentode.koren.kg2")),
            ("kp", value("tube_models.7199.pentode.koren.kp")),
            ("kvb", value("tube_models.7199.pentode.koren.kvb")),
        ];
        let present_7199_pentode = required_7199_pentode
            .iter()
            .filter(|(_, value)| value.is_some())
            .count();
        circuit.tube_models.koren_7199_pentode = match present_7199_pentode {
            0 => None,
            6 => Some(KorenPentodeParameters {
                mu: required_7199_pentode[0].1.expect("presente"),
                ex: required_7199_pentode[1].1.expect("presente"),
                kg1: required_7199_pentode[2].1.expect("presente"),
                kg2: required_7199_pentode[3].1.expect("presente"),
                kp: required_7199_pentode[4].1.expect("presente"),
                kvb: required_7199_pentode[5].1.expect("presente"),
                vct: value("tube_models.7199.pentode.koren.vct").unwrap_or(0.0),
                grid_conduction_threshold_v: value("tube_models.7199.pentode.grid.threshold_v")
                    .unwrap_or(0.0),
                grid_conduction_resistance_ohms: value(
                    "tube_models.7199.pentode.grid.resistance_ohms",
                )
                .unwrap_or(500.0),
            }),
            _ => {
                return Err(ConfigError::Calibration {
                    message: "modelo Koren del pentodo 7199 incompleto: se exigen mu, ex, kg1, \
                              kg2, kp y kvb"
                        .to_owned(),
                })
            }
        };

        let required_7199_triode = [
            ("mu", value("tube_models.7199.triode.koren.mu")),
            ("ex", value("tube_models.7199.triode.koren.ex")),
            ("kg1", value("tube_models.7199.triode.koren.kg1")),
            ("kp", value("tube_models.7199.triode.koren.kp")),
            ("kvb", value("tube_models.7199.triode.koren.kvb")),
        ];
        let present_7199_triode = required_7199_triode
            .iter()
            .filter(|(_, value)| value.is_some())
            .count();
        circuit.tube_models.koren_7199_triode = match present_7199_triode {
            0 => None,
            5 => Some(KorenTriodeParameters {
                mu: required_7199_triode[0].1.expect("presente"),
                ex: required_7199_triode[1].1.expect("presente"),
                kg1: required_7199_triode[2].1.expect("presente"),
                kp: required_7199_triode[3].1.expect("presente"),
                kvb: required_7199_triode[4].1.expect("presente"),
                vct: value("tube_models.7199.triode.koren.vct").unwrap_or(0.0),
                grid_conduction_threshold_v: value("tube_models.7199.triode.grid.threshold_v")
                    .unwrap_or(-0.3),
                grid_conduction_resistance_ohms: value(
                    "tube_models.7199.triode.grid.resistance_ohms",
                )
                .unwrap_or(1_000.0),
            }),
            _ => {
                return Err(ConfigError::Calibration {
                    message: "modelo Koren del triodo 7199 incompleto: se exigen mu, ex, kg1, kp \
                              y kvb"
                        .to_owned(),
                })
            }
        };

        circuit.tube_models.preamp_supply_v = value("tube_models.preamp_supply_v");
        Ok(())
    }

    /// Comprueba revisión objetivo y que no se redefinan nominales protegidos.
    pub fn validate(self) -> Result<Self, ConfigError> {
        if self.target_revision != crate::EXPECTED_SCHEMATIC_REVISION {
            return Err(ConfigError::Calibration {
                message: format!(
                    "target_revision '{}' distinta de '{}'",
                    self.target_revision,
                    crate::EXPECTED_SCHEMATIC_REVISION
                ),
            });
        }
        for (key, entry) in self.assumptions.iter().chain(self.measured.iter()) {
            if PROTECTED_PREFIXES
                .iter()
                .any(|prefix| key.starts_with(prefix))
            {
                return Err(ConfigError::Calibration {
                    message: format!(
                        "'{key}' intenta redefinir un nominal protegido del JSON canónico"
                    ),
                });
            }
            if entry.unit.trim().is_empty() {
                return Err(ConfigError::Calibration {
                    message: format!("'{key}' no declara unidad"),
                });
            }
            if !entry.value.is_finite() {
                return Err(ConfigError::Calibration {
                    message: format!("'{key}' no es finito"),
                });
            }
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{
        "target_revision": "1967-05",
        "notes": "prueba",
        "assumptions": {
            "output_transformer.winding_inductance_h": {
                "value": 12.0,
                "unit": "H",
                "origin": "estimated",
                "method": "ajuste por respuesta en baja frecuencia",
                "date": "2026-10-08"
            }
        },
        "measured": {},
        "provenance": ["medición propia"]
    }"#;

    #[test]
    fn accepts_valid_overlay() {
        let file = CalibrationFile::parse(VALID).expect("overlay válido");
        assert_eq!(file.assumptions.len(), 1);
    }

    #[test]
    fn rejects_protected_keys() {
        let json = VALID.replace(
            "output_transformer.winding_inductance_h",
            "power_stage.plate_voltage_v_nominal",
        );
        assert!(matches!(
            CalibrationFile::parse(&json),
            Err(ConfigError::Calibration { .. })
        ));
    }

    #[test]
    fn rejects_wrong_target_revision() {
        let json = VALID.replace("1967-05", "1966-01");
        assert!(matches!(
            CalibrationFile::parse(&json),
            Err(ConfigError::Calibration { .. })
        ));
    }
}
