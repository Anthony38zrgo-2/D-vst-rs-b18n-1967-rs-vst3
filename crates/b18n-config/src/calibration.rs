//! Overlay de calibración, separado del JSON canónico.
//!
//! Cualquier valor no respaldado por la documentación histórica se registra
//! aquí con unidad, origen, método y fecha. El overlay **no** puede modificar
//! los nominales protegidos (resistencias, condensadores, tensiones).

use std::collections::BTreeMap;

use b18n_dsp::circuit::{CircuitDefinition, KorenPentodeParameters, KorenTriodeParameters};
use b18n_dsp::stages::speaker_impedance::LumpedSpeakerParameters;
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
                screen_ratio_floor: value("tube_models.7027a.koren.screen_ratio_floor")
                    .unwrap_or(0.0),
                screen_ratio_boost: value("tube_models.7027a.koren.screen_ratio_boost")
                    .unwrap_or(0.0),
                screen_ratio_volts: value("tube_models.7027a.koren.screen_ratio_volts")
                    .unwrap_or(50.0),
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
                screen_ratio_floor: value("tube_models.7199.pentode.koren.screen_ratio_floor")
                    .unwrap_or(0.0),
                screen_ratio_boost: value("tube_models.7199.pentode.koren.screen_ratio_boost")
                    .unwrap_or(0.0),
                screen_ratio_volts: value("tube_models.7199.pentode.koren.screen_ratio_volts")
                    .unwrap_or(50.0),
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

        let mut caps = [0.0; 9];
        let keys = ["7199.pentode", "7199.triode", "7027a"];
        let mut count = 0;
        for (tube, key) in keys.iter().enumerate() {
            for (electrode, name) in ["gp", "gk", "pk"].iter().enumerate() {
                if let Some(v) = value(&format!("tube_models.{key}.capacitance.{name}")) {
                    if v <= 0.0 {
                        return Err(ConfigError::Calibration {
                            message: "electrode capacitance must be positive".into(),
                        });
                    }
                    caps[tube * 3 + electrode] = v;
                    count += 1;
                }
            }
        }
        use b18n_dsp::circuit::{ElectrodeCapacitances, TubeCapacitances};
        let electrode = |i: usize| ElectrodeCapacitances {
            gp: caps[i],
            gk: caps[i + 1],
            pk: caps[i + 2],
        };
        circuit.tube_models.capacitances = match count {
            0 => None,
            9 => Some(TubeCapacitances {
                pentode_7199: electrode(0),
                triode_7199: electrode(3),
                power_7027a: electrode(6),
            }),
            _ => {
                return Err(ConfigError::Calibration {
                    message: "incomplete electrode capacitances: all nine are required".into(),
                })
            }
        };
        circuit.tube_models.preamp_supply_v = value("tube_models.preamp_supply_v");
        // Dinámica de fuente B18-012 (RC + rectificador promedio, `estimated`).
        // Claves `supply.*`: no colisionan con los nominales protegidos
        // (`power_supply.*`).
        if let Some(series) = value("supply.rectifier.series_ohms") {
            circuit.power_supply.rectifier_series_ohms = Some(series);
        }
        if let Some(series) = value("supply.bias.series_ohms") {
            circuit.power_supply.bias_series_ohms = Some(series);
        }
        // Parásitos lineales del OT-471 (B18-013, `estimated`): sustituyen los
        // `null` del JSON canónico sin tocarlo. Los tres juntos habilitan el
        // modo lineal del transformador; a medias es un error explícito.
        if let Some(value) = value("output_transformer.winding_inductance_h") {
            circuit.output_transformer.winding_inductance_h = Some(value);
        }
        if let Some(value) = value("output_transformer.leakage_inductance_h") {
            circuit.output_transformer.leakage_inductance_h = Some(value);
        }
        if let Some(value) = value("output_transformer.parasitic_capacitance_f") {
            circuit.output_transformer.parasitic_capacitance_f = Some(value);
        }
        if let Some(value) = value("output_transformer.core_loss_ohms") {
            circuit.output_transformer.core_loss_ohms = Some(value);
        }
        if let Some(value) = value("output_transformer.magnetizing_current_limit_a") {
            circuit.output_transformer.magnetizing_current_limit_a = Some(value);
        }
        // Equivalente RLC estimado del altavoz (B18-015, `estimated`): red
        // Re + Le + (Res ∥ Les ∥ Ces). Los cinco parámetros juntos habilitan
        // la carga reactiva opcional; a medias es un error explícito (no se
        // rellena en silencio).
        let speaker_keys = [
            (
                "voice_coil_resistance_ohms",
                value("speaker.reactive_load.voice_coil_resistance_ohms"),
            ),
            (
                "voice_coil_inductance_h",
                value("speaker.reactive_load.voice_coil_inductance_h"),
            ),
            (
                "motional_resistance_ohms",
                value("speaker.reactive_load.motional_resistance_ohms"),
            ),
            (
                "motional_inductance_h",
                value("speaker.reactive_load.motional_inductance_h"),
            ),
            (
                "motional_capacitance_farads",
                value("speaker.reactive_load.motional_capacitance_farads"),
            ),
        ];
        let speaker_present = speaker_keys
            .iter()
            .filter(|(_, entry)| entry.is_some())
            .count();
        circuit.speaker.lumped_equivalent = match speaker_present {
            0 => None,
            5 => {
                let parameters = LumpedSpeakerParameters {
                    voice_coil_resistance_ohms: speaker_keys[0].1.expect("presente"),
                    voice_coil_inductance_h: speaker_keys[1].1.expect("presente"),
                    motional_resistance_ohms: speaker_keys[2].1.expect("presente"),
                    motional_inductance_h: speaker_keys[3].1.expect("presente"),
                    motional_capacitance_farads: speaker_keys[4].1.expect("presente"),
                };
                if !parameters.is_physically_valid() {
                    return Err(ConfigError::Calibration {
                        message: "el equivalente RLC del altavoz (speaker.reactive_load.*) tiene \
                                  parámetros no positivos o no finitos"
                            .to_owned(),
                    });
                }
                Some(parameters)
            }
            _ => {
                return Err(ConfigError::Calibration {
                    message: "equivalente RLC del altavoz incompleto: se exigen los cinco \
                              parámetros de speaker.reactive_load.*"
                        .to_owned(),
                })
            }
        };
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

    fn base_circuit() -> CircuitDefinition {
        let effective =
            crate::apply_embedded_errata(crate::EMBEDDED_REFERENCE_JSON).expect("errata aplicable");
        let raw = crate::parse_reference(&effective).expect("canónico válido");
        crate::convert_to_circuit(&raw).expect("conversión")
    }

    const SPEAKER_COMPLETE: &str = r#"{
        "target_revision": "1967-05",
        "assumptions": {
            "speaker.reactive_load.voice_coil_resistance_ohms": {
                "value": 6.5, "unit": "ohm", "origin": "estimated"
            },
            "speaker.reactive_load.voice_coil_inductance_h": {
                "value": 0.0008, "unit": "H", "origin": "estimated"
            },
            "speaker.reactive_load.motional_resistance_ohms": {
                "value": 23.5, "unit": "ohm", "origin": "estimated"
            },
            "speaker.reactive_load.motional_inductance_h": {
                "value": 0.0693, "unit": "H", "origin": "estimated"
            },
            "speaker.reactive_load.motional_capacitance_farads": {
                "value": 1.806e-4, "unit": "F", "origin": "estimated"
            }
        }
    }"#;

    #[test]
    fn applies_complete_speaker_reactive_load() {
        let file = CalibrationFile::parse(SPEAKER_COMPLETE).expect("overlay válido");
        let mut circuit = base_circuit();
        file.apply_to(&mut circuit).expect("aplicable");
        let parameters = circuit
            .speaker
            .lumped_equivalent
            .expect("equivalente presente");
        assert!((parameters.resonance_hz() - 45.0).abs() < 0.5);
    }

    #[test]
    fn rejects_partial_speaker_reactive_load() {
        let partial = SPEAKER_COMPLETE.replace(
            ",\n            \"speaker.reactive_load.motional_capacitance_farads\": {\n                \"value\": 1.806e-4, \"unit\": \"F\", \"origin\": \"estimated\"\n            }",
            "",
        );
        assert_ne!(partial, SPEAKER_COMPLETE, "el recorte debe aplicarse");
        let file = CalibrationFile::parse(&partial).expect("JSON válido");
        let mut circuit = base_circuit();
        assert!(matches!(
            file.apply_to(&mut circuit),
            Err(ConfigError::Calibration { .. })
        ));
    }

    #[test]
    fn rejects_degenerate_speaker_reactive_load() {
        let invalid = SPEAKER_COMPLETE.replace("\"value\": 23.5", "\"value\": 0.0");
        let file = CalibrationFile::parse(&invalid).expect("JSON válido");
        let mut circuit = base_circuit();
        assert!(matches!(
            file.apply_to(&mut circuit),
            Err(ConfigError::Calibration { .. })
        ));
    }
    #[test]
    fn electrode_capacitances_require_complete_positive_data() {
        let json = include_str!("../../../assets/models/b18n_1967_calibration.json");
        let complete = CalibrationFile::parse(json).expect("overlay");
        let mut circuit = base_circuit();
        complete
            .apply_to(&mut circuit)
            .expect("complete capacitances");
        let caps = circuit.tube_models.capacitances.expect("all three tubes");
        assert_eq!(caps.power_7027a.gp, 1.5e-12);
        let mut partial = complete.clone();
        partial
            .assumptions
            .remove("tube_models.7027a.capacitance.gp");
        assert!(partial.apply_to(&mut base_circuit()).is_err());
        let mut zero = complete.clone();
        zero.assumptions
            .get_mut("tube_models.7199.pentode.capacitance.gk")
            .expect("key")
            .value = 0.0;
        assert!(zero.apply_to(&mut base_circuit()).is_err());
        let mut absent = complete;
        absent
            .assumptions
            .retain(|key, _| !key.contains(".capacitance."));
        absent
            .apply_to(&mut circuit)
            .expect("legacy overlay without capacitances");
        assert!(circuit.tube_models.capacitances.is_none());
    }
}
