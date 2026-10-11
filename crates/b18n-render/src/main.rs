//! `b18n-render`: procesa archivos WAV sin DAW y genera reportes.
//!
//! Hito A: el motor es un passthrough determinista; este binario ya fija la
//! interfaz, la trazabilidad del modelo y las métricas básicas.

mod report;
mod sweep;
mod wav_io;

use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};

use b18n_config::{parse_reference, CircuitDefinition};
use b18n_dsp::{B18nEngine, InputChannel, ProcessSpec, RuntimeControls, ToneStackDesign};
use report::{
    metrics, now_utc, AudioFileReport, ModelReport, RenderReport, SettingsReport,
    ToneMeasurementReport,
};

use sha2::{Digest, Sha256};

/// Entrada histórica a la que enrutar el procesado.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum ChannelArg {
    Bass,
    Guitar,
    Instrument,
}

impl ChannelArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::Bass => "bass",
            Self::Guitar => "guitar",
            Self::Instrument => "instrument",
        }
    }

    fn to_input_channel(self) -> InputChannel {
        match self {
            Self::Bass => InputChannel::Bass,
            Self::Guitar => InputChannel::Guitar,
            Self::Instrument => InputChannel::Instrument,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CabinetArg {
    On,
    Off,
}

impl CabinetArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
        }
    }
}

/// Mediciones adicionales al procesado del WAV.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum MeasureArg {
    /// Curvas de la red de tono: 25 combinaciones Bass/Treble.
    Tone,
}

/// Procesado offline del B-18N 1967.
#[derive(Debug, Parser)]
#[command(name = "b18n-render", version, about, long_about = None)]
struct Cli {
    /// WAV de entrada.
    #[arg(long)]
    input: PathBuf,
    /// WAV de salida (float 32).
    #[arg(long)]
    output: PathBuf,
    /// Modelo JSON; por defecto usa el JSON canónico embebido.
    #[arg(long)]
    model: Option<PathBuf>,
    /// Sample rate esperado; debe coincidir con el WAV (no hay remuestreo aún).
    #[arg(long)]
    sample_rate: Option<u32>,
    /// Factor de sobremuestreo: 1, 4 u 8.
    #[arg(long, default_value_t = 4)]
    oversampling: u8,
    /// Entrada histórica seleccionada.
    #[arg(long, value_enum, default_value_t = ChannelArg::Bass)]
    channel: ChannelArg,
    /// IR de gabinete (moderna; Hito B: sin gabinete).
    #[arg(long, value_enum, default_value_t = CabinetArg::Off)]
    cabinet: CabinetArg,
    /// Mediciones adicionales (p. ej. `tone`).
    #[arg(long, value_enum)]
    measure: Option<MeasureArg>,
    /// Destino del CSV de medición; por defecto `<output>.tone_curves.csv`.
    #[arg(long)]
    measure_output: Option<PathBuf>,
    /// Ruta del reporte JSON; sin ella se imprime en stdout.
    #[arg(long)]
    report: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<(), Box<dyn Error>> {
    let LoadedModel {
        circuit,
        source,
        input_json,
        errata_applied,
        effective_json,
    } = load_model(cli.model.as_deref())?;

    let input = wav_io::read_wav(&cli.input)?;
    if input.channels.is_empty() {
        return Err(invalid_input("el WAV de entrada no tiene canales").into());
    }
    if let Some(requested) = cli.sample_rate {
        if requested != input.sample_rate_hz {
            return Err(invalid_input(format!(
                "--sample-rate {requested} no coincide con el WAV ({} Hz); remuestreo no implementado",
                input.sample_rate_hz
            ))
            .into());
        }
    }

    let tone_measurement = match cli.measure {
        Some(MeasureArg::Tone) => Some(measure_tone(cli, &circuit)?),
        None => None,
    };

    let spec = ProcessSpec {
        sample_rate_hz: f64::from(input.sample_rate_hz),
        max_block_frames: input.frames().max(1),
        oversampling: cli.oversampling,
    };

    let mut engines = Vec::with_capacity(input.channels.len());
    for _ in 0..input.channels.len() {
        let mut engine = B18nEngine::new(circuit.clone(), spec)?;
        let controls = RuntimeControls {
            channel: cli.channel.to_input_channel(),
            cabinet_enabled: cli.cabinet == CabinetArg::On,
            ..RuntimeControls::default()
        };
        engine.set_controls(controls);
        engines.push(engine);
    }

    let mut output_channels = Vec::with_capacity(input.channels.len());
    for (samples, engine) in input.channels.iter().zip(engines.iter_mut()) {
        let mut processed = vec![0.0_f32; samples.len()];
        engine.process_mono(samples, &mut processed);
        output_channels.push(processed);
    }

    if let Some(parent) = cli.output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    wav_io::write_wav_f32(&cli.output, input.sample_rate_hz, &output_channels)?;

    let report = RenderReport {
        tool: "b18n-render",
        algorithm_version: report::ALGORITHM_VERSION,
        timestamp: now_utc(),
        model: ModelReport {
            source,
            sha256: sha256_hex(input_json.as_bytes()),
            errata_sha256: errata_applied
                .then(|| sha256_hex(b18n_config::EMBEDDED_ERRATA_JSON.as_bytes())),
            effective_sha256: sha256_hex(effective_json.as_bytes()),
            schematic_revision: circuit.identity.schematic_revision.clone(),
        },
        input: AudioFileReport {
            path: cli.input.display().to_string(),
            sample_rate_hz: input.sample_rate_hz,
            channels: input.channels.len(),
            frames: input.frames(),
        },
        output: AudioFileReport {
            path: cli.output.display().to_string(),
            sample_rate_hz: input.sample_rate_hz,
            channels: output_channels.len(),
            frames: output_channels.first().map_or(0, Vec::len),
        },
        settings: SettingsReport {
            channel: cli.channel.as_str(),
            oversampling: cli.oversampling,
            cabinet: cli.cabinet.as_str(),
        },
        metrics: metrics(&output_channels),
        fidelity: circuit.fidelity.as_str().to_owned(),
        latency_samples: engines.first().map_or(0, B18nEngine::latency_samples),
        tone_measurement,
    };

    let json = report.to_json()?;
    match &cli.report {
        Some(path) => {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)?;
                }
            }
            fs::write(path, format!("{json}\n"))?;
        }
        None => println!("{json}"),
    }

    Ok(())
}

fn measure_tone(
    cli: &Cli,
    circuit: &CircuitDefinition,
) -> Result<ToneMeasurementReport, Box<dyn Error>> {
    let design = ToneStackDesign::from_circuit(circuit)
        .map_err(|error| invalid_input(format!("red de tono incompleta: {error}")))?;
    let csv_path = cli
        .measure_output
        .clone()
        .unwrap_or_else(|| cli.output.with_extension("tone_curves.csv"));
    if let Some(parent) = csv_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let summary = sweep::write_tone_curves(&csv_path, &design)
        .map_err(|error| invalid_input(format!("fallo en el barrido de tono: {error}")))?;
    Ok(ToneMeasurementReport {
        csv: csv_path.display().to_string(),
        combinations: sweep::POSITIONS.len() * sweep::POSITIONS.len(),
        frequencies_per_curve: sweep::FREQUENCIES_HZ.len(),
        bass_boost_db: summary.bass_boost_db,
        bass_cut_db: summary.bass_cut_db,
        treble_boost_db: summary.treble_boost_db,
        treble_cut_db: summary.treble_cut_db,
    })
}

/// Modelo cargado con su trazabilidad de huellas (entrada/errata/efectivo).
struct LoadedModel {
    circuit: CircuitDefinition,
    source: String,
    input_json: String,
    errata_applied: bool,
    effective_json: String,
}

fn load_model(path: Option<&std::path::Path>) -> Result<LoadedModel, Box<dyn Error>> {
    match path {
        Some(path) => {
            let json = fs::read_to_string(path).map_err(|error| {
                invalid_input(format!("no se pudo leer {}: {error}", path.display()))
            })?;
            // La errata está enlazada al SHA-256 canónico: solo se aplica a ese
            // archivo exacto, nunca a modelos de usuario distintos.
            let is_canonical =
                sha256_hex(json.as_bytes()) == b18n_config::EXPECTED_REFERENCE_SHA256;
            let effective = if is_canonical {
                b18n_config::apply_embedded_errata(&json)?
            } else {
                json.clone()
            };
            let circuit = circuit_from_json(&effective)?;
            Ok(LoadedModel {
                circuit,
                source: path.display().to_string(),
                input_json: json,
                errata_applied: is_canonical,
                effective_json: effective,
            })
        }
        None => {
            let effective =
                b18n_config::apply_embedded_errata(b18n_config::EMBEDDED_REFERENCE_JSON)?;
            let circuit = circuit_from_json(&effective)?;
            Ok(LoadedModel {
                circuit,
                source: "<embedded>".to_owned(),
                input_json: b18n_config::EMBEDDED_REFERENCE_JSON.to_owned(),
                errata_applied: true,
                effective_json: effective,
            })
        }
    }
}

/// Valida y convierte un JSON efectivo (errata ya aplicada) con su calibración.
fn circuit_from_json(json: &str) -> Result<CircuitDefinition, Box<dyn Error>> {
    let raw = parse_reference(json)?;
    let mut circuit = b18n_config::convert_to_circuit(&raw)?;
    b18n_config::apply_embedded_calibration(&mut circuit)?;
    Ok(circuit)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
