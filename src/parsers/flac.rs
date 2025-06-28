use std::fs::File;
use std::io::{Read, Result, Seek, SeekFrom};
use std::path::Path;

use super::{AudioMetadata, Bitrate};

pub(crate) struct FlacParser {
    sample_rate: u32,
    total_samples: u64,
    _channels: u8,
    bits_per_sample: u8,
    min_frame_size: u32,
    max_frame_size: u32,
    audio_start_pos: u64,
    file_size: u64,
}

impl AudioMetadata for FlacParser {
    fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(path)?;

        // Check FLAC signature
        let mut signature = [0; 4];
        file.read_exact(&mut signature)?;
        if &signature != b"fLaC" {
            return Err(std::io::Error::other("Not a FLAC file"));
        }

        let mut audio_start_pos = 4; // Position after signature
        let mut streaminfo = None;

        // Process metadata blocks
        loop {
            file.seek(SeekFrom::Start(audio_start_pos))?;
            let mut block_header = [0; 4];
            file.read_exact(&mut block_header)?;

            let is_last = (block_header[0] & 0x80) != 0;
            let block_type = block_header[0] & 0x7F;
            let length = u32::from_be_bytes([0, block_header[1], block_header[2], block_header[3]]);

            audio_start_pos += 4 + length as u64;

            if block_type == 0 {
                // STREAMINFO
                let mut data = [0; 34];
                file.read_exact(&mut data)?;

                // Parse sample rate (20 bits)
                let sample_rate = (u32::from(data[10]) << 12)
                    | (u32::from(data[11]) << 4)
                    | (u32::from(data[12]) >> 4);

                // Parse total samples (36 bits)
                let total_samples = (u64::from(data[13] & 0x0F) << 32)
                    | u64::from(data[14]) << 24
                    | u64::from(data[15]) << 16
                    | u64::from(data[16]) << 8
                    | u64::from(data[17]);

                // Parse other fields
                let channels = ((data[12] & 0x0E) >> 1) + 1;
                let bits_per_sample = ((data[12] & 0x01) << 4) | (data[13] >> 4) + 1;
                let min_frame_size =
                    (u32::from(data[4]) << 16) | (u32::from(data[5]) << 8) | u32::from(data[6]);
                let max_frame_size =
                    (u32::from(data[7]) << 16) | (u32::from(data[8]) << 8) | u32::from(data[9]);

                streaminfo = Some((
                    sample_rate,
                    total_samples,
                    channels,
                    bits_per_sample,
                    min_frame_size,
                    max_frame_size,
                ));
            }

            if is_last {
                break;
            }
        }

        let (sample_rate, total_samples, _channels, bits_per_sample, min_frame_size, max_frame_size) =
            streaminfo.ok_or(std::io::Error::other("STREAMINFO block not found"))?;

        let file_size = file.seek(SeekFrom::End(0))?;

        Ok(Self {
            sample_rate,
            total_samples,
            _channels,
            bits_per_sample,
            min_frame_size,
            max_frame_size,
            audio_start_pos,
            file_size,
        })
    }

    fn bitrate(&self) -> Bitrate {
        let duration = self.duration();
        let audio_size = self.file_size - self.audio_start_pos;
        Bitrate::Variable(((audio_size as f64 * 8.0) / (duration * 1000.0)) as u16)
    }

    fn duration(&self) -> f64 {
        self.total_samples as f64 / self.sample_rate as f64
    }

    fn is_vbr(&self) -> bool {
        self.min_frame_size != self.max_frame_size
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn bit_depth(&self) -> Option<u16> {
        Some(self.bits_per_sample as u16)
    }
}
