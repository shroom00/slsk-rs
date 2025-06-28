use std::fs::File;
use std::io::{Read, Result, Seek, SeekFrom};
use std::path::Path;

use super::{AudioMetadata, Bitrate};

#[derive(Debug)]
pub(crate) struct WavParser {
    sample_rate: u32,
    bits_per_sample: u16,
    _channels: u16,
    byte_rate: u32,
    data_size: u32,
}

impl WavParser {
    fn find_chunk(file: &mut File, chunk_id: &[u8; 4]) -> Result<ChunkInfo> {
        // Start searching from the beginning of the file (after RIFF header)
        file.seek(SeekFrom::Start(12))?;

        loop {
            let mut chunk_header = [0u8; 8];
            file.read_exact(&mut chunk_header)?;

            let current_chunk_id = &chunk_header[0..4];
            let chunk_size = u32::from_le_bytes([
                chunk_header[4],
                chunk_header[5],
                chunk_header[6],
                chunk_header[7],
            ]);

            if current_chunk_id == chunk_id {
                return Ok(ChunkInfo {
                    size: chunk_size,
                    _position: file.stream_position()?,
                });
            } else {
                // Skip to next chunk
                file.seek(SeekFrom::Current(chunk_size as i64))?;
            }
        }
    }
}

struct ChunkInfo {
    size: u32,
    _position: u64,
}

impl AudioMetadata for WavParser {
    fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(path)?;

        // Read RIFF header
        let mut riff_header = [0u8; 12];
        file.read_exact(&mut riff_header)?;

        // Check if it's a WAV file
        if &riff_header[0..4] != b"RIFF" || &riff_header[8..12] != b"WAVE" {
            return Err(std::io::Error::other("Not a valid WAV file"));
        }

        // Find and read the fmt chunk
        let fmt_chunk = Self::find_chunk(&mut file, b"fmt ")?;

        // Parse fmt chunk
        let mut fmt_data = vec![0u8; fmt_chunk.size as usize];
        file.read_exact(&mut fmt_data)?;

        // Check audio format (1 = PCM)
        let audio_format = u16::from_le_bytes([fmt_data[0], fmt_data[1]]);
        if audio_format != 1 {
            return Err(std::io::Error::other(
                "Only PCM format WAV files are supported",
            ));
        }

        let _channels = u16::from_le_bytes([fmt_data[2], fmt_data[3]]);
        let sample_rate = u32::from_le_bytes([fmt_data[4], fmt_data[5], fmt_data[6], fmt_data[7]]);
        let byte_rate = u32::from_le_bytes([fmt_data[8], fmt_data[9], fmt_data[10], fmt_data[11]]);
        let bits_per_sample = u16::from_le_bytes([fmt_data[14], fmt_data[15]]);

        // Find and read the data chunk
        let data_chunk = Self::find_chunk(&mut file, b"data")?;

        Ok(WavParser {
            sample_rate,
            bits_per_sample,
            _channels,
            byte_rate,
            data_size: data_chunk.size,
        })
    }

    fn bitrate(&self) -> Bitrate {
        // WAV is always constant bitrate
        let kbps = (self.byte_rate * 8) / 1000;
        Bitrate::Constant(kbps as u16)
    }

    fn duration(&self) -> f64 {
        let bytes_per_second = self.byte_rate as f64;
        let total_bytes = self.data_size as f64;
        total_bytes / bytes_per_second
    }

    fn is_vbr(&self) -> bool {
        false // WAV is always constant bitrate
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn bit_depth(&self) -> Option<u16> {
        Some(self.bits_per_sample)
    }
}
