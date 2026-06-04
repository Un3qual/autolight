use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::Path;

const WAVE_FORMAT_PCM: u16 = 1;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xfffe;
pub(super) const WAVE_SUBFORMAT_PCM: [u8; 16] = [
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];
const WAVE_SUBFORMAT_IEEE_FLOAT: [u8; 16] = [
    0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];

#[derive(Debug, Clone, PartialEq)]
pub(super) struct AudioMetadata {
    pub(super) duration: f64,
    pub(super) sample_rate: u32,
    pub(super) channels: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct AudioInspection {
    pub(super) metadata: AudioMetadata,
    pub(super) fingerprint: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct WavMonoSamples {
    pub(super) sample_rate: u32,
    pub(super) samples: Vec<f32>,
}

pub(super) fn inspect_wav_file(path: &Path) -> Result<AudioInspection, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut reader = HashedReader::new(BufReader::new(file));
    read_wav_header(&mut reader)?;
    let chunks = read_wav_chunks(&mut reader)?;
    Ok(AudioInspection {
        metadata: chunks.audio_metadata()?,
        fingerprint: reader.finish(),
    })
}

pub(super) fn write_silent_wav(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    frames: u32,
) -> Result<(), String> {
    let bytes_per_sample = 2_u16;
    let data_size = frames
        .checked_mul(u32::from(channels))
        .and_then(|value| value.checked_mul(u32::from(bytes_per_sample)))
        .ok_or_else(|| "demo WAV is too large".to_string())?;
    let riff_size = 36_u32
        .checked_add(data_size)
        .ok_or_else(|| "demo WAV is too large".to_string())?;
    let byte_rate = sample_rate
        .checked_mul(u32::from(channels))
        .and_then(|value| value.checked_mul(u32::from(bytes_per_sample)))
        .ok_or_else(|| "demo WAV is too large".to_string())?;
    let block_align = channels
        .checked_mul(bytes_per_sample)
        .ok_or_else(|| "demo WAV is too large".to_string())?;
    let mut file = File::create(path).map_err(|error| error.to_string())?;
    file.write_all(b"RIFF").map_err(|error| error.to_string())?;
    file.write_all(&riff_size.to_le_bytes())
        .map_err(|error| error.to_string())?;
    file.write_all(b"WAVEfmt ")
        .map_err(|error| error.to_string())?;
    file.write_all(&16_u32.to_le_bytes())
        .map_err(|error| error.to_string())?;
    file.write_all(&WAVE_FORMAT_PCM.to_le_bytes())
        .map_err(|error| error.to_string())?;
    file.write_all(&channels.to_le_bytes())
        .map_err(|error| error.to_string())?;
    file.write_all(&sample_rate.to_le_bytes())
        .map_err(|error| error.to_string())?;
    file.write_all(&byte_rate.to_le_bytes())
        .map_err(|error| error.to_string())?;
    file.write_all(&block_align.to_le_bytes())
        .map_err(|error| error.to_string())?;
    file.write_all(&16_u16.to_le_bytes())
        .map_err(|error| error.to_string())?;
    file.write_all(b"data").map_err(|error| error.to_string())?;
    file.write_all(&data_size.to_le_bytes())
        .map_err(|error| error.to_string())?;
    let frame = vec![0_u8; usize::from(block_align)];
    for _ in 0..frames {
        file.write_all(&frame).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(super) fn read_wav_mono_samples(path: &Path) -> Result<WavMonoSamples, String> {
    let file = File::open(path).map_err(|error| error.to_string())?;
    let mut reader = HashedReader::new(BufReader::new(file));
    read_wav_header(&mut reader)?;
    let mut format = None;
    let mut data = Vec::default();
    while let Some(chunk) = read_wav_chunk_header(&mut reader)? {
        if chunk.id == *b"fmt " && chunk.size >= 16 {
            format = Some(read_wav_format(&mut reader, chunk.size)?);
        } else if chunk.id == *b"data" {
            let data_size = usize::try_from(chunk.size).map_err(|_| "WAV data chunk too large")?;
            if data_size == 0 {
                data.clear();
            } else {
                let mut chunk_data = vec![0_u8; data_size];
                reader
                    .read_exact(&mut chunk_data)
                    .map_err(|error| error.to_string())?;
                data = chunk_data;
            }
        } else {
            reader.skip_bytes(chunk.size)?;
        }
        if chunk.size % 2 == 1 {
            reader.skip_bytes(1)?;
        }
    }
    let format = format.ok_or_else(|| "invalid WAV metadata".to_string())?;
    Ok(WavMonoSamples {
        sample_rate: format.sample_rate,
        samples: decode_wav_mono_samples(&format, &data)?,
    })
}

fn read_wav_header(reader: &mut HashedReader<impl Read>) -> Result<(), String> {
    let mut header = [0_u8; 12];
    reader
        .read_exact(&mut header)
        .map_err(|error| error.to_string())?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Err("unsupported audio file: expected WAV".to_string());
    }
    Ok(())
}

fn read_wav_chunks(reader: &mut HashedReader<impl Read>) -> Result<WavChunkMetadata, String> {
    let mut metadata = WavChunkMetadata::default();
    while let Some(chunk) = read_wav_chunk_header(reader)? {
        if chunk.id == *b"fmt " && chunk.size >= 16 {
            metadata.format = Some(read_wav_format(reader, chunk.size)?);
        } else if chunk.id == *b"data" {
            metadata.has_data_chunk = true;
            metadata.data_bytes = chunk.size;
            reader.skip_bytes(chunk.size)?;
        } else {
            reader.skip_bytes(chunk.size)?;
        }
        if chunk.size % 2 == 1 {
            reader.skip_bytes(1)?;
        }
    }
    Ok(metadata)
}

fn read_wav_chunk_header(
    reader: &mut HashedReader<impl Read>,
) -> Result<Option<WavChunkHeader>, String> {
    let mut chunk_header = [0_u8; 8];
    match reader.read_exact(&mut chunk_header) {
        Ok(()) => Ok(Some(WavChunkHeader {
            id: [
                chunk_header[0],
                chunk_header[1],
                chunk_header[2],
                chunk_header[3],
            ],
            size: u32::from_le_bytes([
                chunk_header[4],
                chunk_header[5],
                chunk_header[6],
                chunk_header[7],
            ]) as u64,
        })),
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn read_wav_format(
    reader: &mut HashedReader<impl Read>,
    chunk_size: u64,
) -> Result<WavFormat, String> {
    let mut fmt = [0_u8; 16];
    reader
        .read_exact(&mut fmt)
        .map_err(|error| error.to_string())?;
    let audio_format = u16::from_le_bytes([fmt[0], fmt[1]]);
    let extension_size = chunk_size - 16;
    let extension_read_size = extension_size.min(24) as usize;
    let mut extension = vec![0_u8; extension_read_size];
    if extension_read_size > 0 {
        reader
            .read_exact(&mut extension)
            .map_err(|error| error.to_string())?;
    }
    let Some(sample_format) = wav_sample_format(audio_format, &extension) else {
        return Err("unsupported WAV encoding".to_string());
    };
    reader.skip_bytes(extension_size - extension_read_size as u64)?;
    Ok(WavFormat {
        sample_format,
        channels: u16::from_le_bytes([fmt[2], fmt[3]]) as u32,
        sample_rate: u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]),
        bits_per_sample: u16::from_le_bytes([fmt[14], fmt[15]]) as u32,
    })
}

struct WavChunkHeader {
    id: [u8; 4],
    size: u64,
}

#[derive(Default)]
struct WavChunkMetadata {
    format: Option<WavFormat>,
    data_bytes: u64,
    has_data_chunk: bool,
}

impl WavChunkMetadata {
    fn audio_metadata(&self) -> Result<AudioMetadata, String> {
        let Some(format) = &self.format else {
            return Err("invalid WAV metadata".to_string());
        };
        if format.sample_rate == 0 || format.channels == 0 || format.bits_per_sample == 0 {
            return Err("invalid WAV metadata".to_string());
        }
        if !self.has_data_chunk || self.data_bytes == 0 {
            return Err("invalid WAV data chunk".to_string());
        }
        if !format.bits_per_sample.is_multiple_of(8) {
            return Err("invalid WAV frame size".to_string());
        }
        let bytes_per_frame = u64::from(format.channels) * u64::from(format.bits_per_sample / 8);
        if bytes_per_frame == 0 {
            return Err("invalid WAV frame size".to_string());
        }
        if !self.data_bytes.is_multiple_of(bytes_per_frame) {
            return Err("invalid WAV frame size".to_string());
        }
        Ok(AudioMetadata {
            duration: self.data_bytes as f64 / (format.sample_rate as f64 * bytes_per_frame as f64),
            sample_rate: format.sample_rate,
            channels: format.channels,
        })
    }
}

#[derive(Clone, Copy)]
enum WavSampleFormat {
    Pcm,
    Float,
}

#[derive(Clone, Copy)]
struct WavFormat {
    sample_format: WavSampleFormat,
    pub(super) channels: u32,
    pub(super) sample_rate: u32,
    bits_per_sample: u32,
}

fn decode_wav_mono_samples(format: &WavFormat, data: &[u8]) -> Result<Vec<f32>, String> {
    if format.sample_rate == 0 || format.channels == 0 || format.bits_per_sample == 0 {
        return Err("invalid WAV metadata".to_string());
    }
    if !format.bits_per_sample.is_multiple_of(8) {
        return Err("invalid WAV frame size".to_string());
    }
    let bytes_per_sample = usize::try_from(format.bits_per_sample / 8)
        .map_err(|_| "invalid WAV frame size".to_string())?;
    let channel_count =
        usize::try_from(format.channels).map_err(|_| "invalid WAV frame size".to_string())?;
    let bytes_per_frame = bytes_per_sample
        .checked_mul(channel_count)
        .filter(|value| *value > 0)
        .ok_or_else(|| "invalid WAV frame size".to_string())?;
    if data.is_empty() {
        return Err("invalid WAV data chunk".to_string());
    }
    if !data.len().is_multiple_of(bytes_per_frame) {
        return Err("invalid WAV frame size".to_string());
    }

    data.chunks_exact(bytes_per_frame)
        .map(|frame| {
            let mut sum = 0.0_f32;
            for channel_index in 0..channel_count {
                let start = channel_index * bytes_per_sample;
                sum += decode_wav_sample(format, &frame[start..start + bytes_per_sample])?;
            }
            Ok(sum / channel_count as f32)
        })
        .collect()
}

fn decode_wav_sample(format: &WavFormat, bytes: &[u8]) -> Result<f32, String> {
    match (format.sample_format, format.bits_per_sample) {
        (WavSampleFormat::Pcm, 8) => Ok((f32::from(bytes[0]) - 128.0) / 128.0),
        (WavSampleFormat::Pcm, 16) => {
            Ok(f32::from(i16::from_le_bytes([bytes[0], bytes[1]])) / 32768.0)
        }
        (WavSampleFormat::Pcm, 24) => {
            let raw = i32::from_le_bytes([
                bytes[0],
                bytes[1],
                bytes[2],
                if bytes[2] & 0x80 == 0 { 0x00 } else { 0xff },
            ]);
            Ok(raw as f32 / 8_388_608.0)
        }
        (WavSampleFormat::Pcm, 32) => {
            Ok(
                i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as f32
                    / 2_147_483_648.0,
            )
        }
        (WavSampleFormat::Float, 32) => Ok(finite_sample(f32::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
        ]))),
        (WavSampleFormat::Float, 64) => {
            let value = f64::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]);
            Ok(if value.is_finite() { value as f32 } else { 0.0 })
        }
        _ => Err("unsupported WAV sample format".to_string()),
    }
}

fn finite_sample(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

fn wav_sample_format(audio_format: u16, extension: &[u8]) -> Option<WavSampleFormat> {
    match audio_format {
        WAVE_FORMAT_PCM => Some(WavSampleFormat::Pcm),
        WAVE_FORMAT_IEEE_FLOAT => Some(WavSampleFormat::Float),
        WAVE_FORMAT_EXTENSIBLE => {
            let subformat = extension.get(8..24)?;
            if subformat == WAVE_SUBFORMAT_PCM {
                Some(WavSampleFormat::Pcm)
            } else if subformat == WAVE_SUBFORMAT_IEEE_FLOAT {
                Some(WavSampleFormat::Float)
            } else {
                None
            }
        }
        _ => None,
    }
}

struct HashedReader<R> {
    inner: R,
    hash: u64,
}

impl<R: Read> HashedReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            hash: 0xcbf29ce484222325_u64,
        }
    }

    fn read_exact(&mut self, buffer: &mut [u8]) -> std::io::Result<()> {
        self.inner.read_exact(buffer)?;
        self.update(buffer);
        Ok(())
    }

    fn skip_bytes(&mut self, mut remaining: u64) -> Result<(), String> {
        let mut buffer = [0_u8; 8192];
        while remaining > 0 {
            let to_read = remaining.min(buffer.len() as u64) as usize;
            self.read_exact(&mut buffer[..to_read])
                .map_err(|error| error.to_string())?;
            remaining -= to_read as u64;
        }
        Ok(())
    }

    fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.hash ^= u64::from(*byte);
            self.hash = self.hash.wrapping_mul(0x100000001b3);
        }
    }

    fn finish(self) -> String {
        format!("{:016x}", self.hash)
    }
}
