//! Lectura y escritura WAV para el renderizador offline.

use std::fmt;
use std::path::Path;

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};

/// Muestras desintercaladas por canal.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioBuffer {
    pub sample_rate_hz: u32,
    pub channels: Vec<Vec<f32>>,
}

impl AudioBuffer {
    #[must_use]
    pub fn frames(&self) -> usize {
        self.channels.first().map_or(0, Vec::len)
    }
}

/// Errores de E/S de audio, con causa explícita.
#[derive(Debug)]
pub enum WavError {
    Io(std::io::Error),
    Hound(hound::Error),
    UnsupportedFormat { bits: u16, format: &'static str },
    NoChannels,
}

impl fmt::Display for WavError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(source) => write!(f, "error de E/S: {source}"),
            Self::Hound(source) => write!(f, "WAV inválido: {source}"),
            Self::UnsupportedFormat { bits, format } => {
                write!(f, "formato WAV no soportado: {format} de {bits} bits")
            }
            Self::NoChannels => write!(f, "el WAV no declara canales"),
        }
    }
}

impl std::error::Error for WavError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::Hound(source) => Some(source),
            _ => None,
        }
    }
}

impl From<std::io::Error> for WavError {
    fn from(source: std::io::Error) -> Self {
        Self::Io(source)
    }
}

impl From<hound::Error> for WavError {
    fn from(source: hound::Error) -> Self {
        Self::Hound(source)
    }
}

/// Lee un WAV PCM entero (16/24/32 bits) o float 32 y lo desintercala.
pub fn read_wav(path: &Path) -> Result<AudioBuffer, WavError> {
    let mut reader = WavReader::open(path)?;
    let spec = reader.spec();
    if spec.channels == 0 {
        return Err(WavError::NoChannels);
    }
    let channel_count = usize::from(spec.channels);
    let mut channels: Vec<Vec<f32>> =
        vec![Vec::with_capacity(reader.len() as usize / channel_count); channel_count];

    match spec.sample_format {
        SampleFormat::Float => {
            if spec.bits_per_sample != 32 {
                return Err(WavError::UnsupportedFormat {
                    bits: spec.bits_per_sample,
                    format: "float",
                });
            }
            for (index, sample) in reader.samples::<f32>().enumerate() {
                channels[index % channel_count].push(sample?);
            }
        }
        SampleFormat::Int => {
            let scale = match spec.bits_per_sample {
                16 => 32_768.0_f32,
                24 => 8_388_608.0_f32,
                32 => 2_147_483_648.0_f32,
                bits => {
                    return Err(WavError::UnsupportedFormat {
                        bits,
                        format: "int",
                    })
                }
            };
            for (index, sample) in reader.samples::<i32>().enumerate() {
                channels[index % channel_count].push(sample? as f32 / scale);
            }
        }
    }

    Ok(AudioBuffer {
        sample_rate_hz: spec.sample_rate,
        channels,
    })
}

/// Escribe un WAV float de 32 bits con las muestras desintercaladas.
pub fn write_wav_f32(
    path: &Path,
    sample_rate_hz: u32,
    channels: &[Vec<f32>],
) -> Result<(), WavError> {
    let channel_count = u16::try_from(channels.len()).map_err(|_| WavError::NoChannels)?;
    if channel_count == 0 {
        return Err(WavError::NoChannels);
    }
    let spec = WavSpec {
        channels: channel_count,
        sample_rate: sample_rate_hz,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let mut writer = WavWriter::create(path, spec)?;
    let frames = channels.iter().map(Vec::len).min().unwrap_or(0);
    for frame in 0..frames {
        for channel in channels {
            writer.write_sample(channel[frame])?;
        }
    }
    writer.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("b18n_render_{}_{}.wav", std::process::id(), name))
    }

    #[test]
    fn roundtrip_preserves_samples() {
        let path = temp_path("roundtrip");
        let original = vec![
            vec![0.0_f32, 0.5, -0.5, 1.0],
            vec![0.25_f32, -0.25, 0.75, -1.0],
        ];
        write_wav_f32(&path, 48_000, &original).expect("escritura");
        let read = read_wav(&path).expect("lectura");
        std::fs::remove_file(&path).ok();

        assert_eq!(read.sample_rate_hz, 48_000);
        assert_eq!(read.channels, original);
    }
}
