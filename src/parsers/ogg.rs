use super::{AudioMetadata, Bitrate};
use std::fs::File;
use std::io::{Read, Result, Seek, SeekFrom};
use std::path::Path;

pub struct OggParser {
    bitrate: Bitrate,
    duration: f64,
    sample_rate: u32,
    bit_depth: Option<u16>,
}

impl AudioMetadata for OggParser {
    fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        // Look for audio streams in the file
        let (sample_rate, bit_depth, is_vbr) = Self::find_audio_stream(&buffer)?;

        // Calculate duration from granule positions
        let duration = Self::calculate_duration(&mut file, sample_rate)?;

        // Calculate bitrate from file size and duration
        file.seek(SeekFrom::Start(0))?;
        let file_size = file.seek(SeekFrom::End(0))? as f64;
        let avg_bitrate = if duration > 0.0 {
            ((file_size * 8.0) / duration / 1000.0) as u16
        } else {
            0
        };

        let bitrate = if is_vbr {
            Bitrate::Variable(avg_bitrate)
        } else {
            Bitrate::Constant(avg_bitrate)
        };

        Ok(OggParser {
            bitrate,
            duration,
            sample_rate,
            bit_depth,
        })
    }

    fn bitrate(&self) -> Bitrate {
        self.bitrate
    }

    fn duration(&self) -> f64 {
        self.duration
    }

    fn is_vbr(&self) -> bool {
        matches!(self.bitrate, Bitrate::Variable(_))
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn bit_depth(&self) -> Option<u16> {
        self.bit_depth
    }
}

impl OggParser {
    fn find_audio_stream(buffer: &[u8]) -> Result<(u32, Option<u16>, bool)> {
        let mut pos = 0;

        // Scan through all OGG pages looking for audio streams
        while pos < buffer.len() {
            if let Some(page_pos) = Self::find_next_page(&buffer[pos..]) {
                pos += page_pos;

                if pos + 27 > buffer.len() {
                    break;
                }

                // Parse page header
                let page_segments = buffer[pos + 26] as usize;
                let data_start = pos + 27 + page_segments;

                if data_start >= buffer.len() {
                    break;
                }

                let data_size: usize = buffer[pos + 27..pos + 27 + page_segments]
                    .iter()
                    .map(|&x| x as usize)
                    .sum();

                if data_start + data_size > buffer.len() {
                    break;
                }

                let data = &buffer[data_start..data_start + data_size];

                // Try to parse this page as an audio codec
                match Self::parse_audio_codec(data) {
                    Ok(audio_info) => {
                        return Ok(audio_info);
                    }
                    Err(_) => ()
                }

                // Move to next page
                pos = data_start + data_size;
            } else {
                break;
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "No audio stream found",
        ))
    }

    fn parse_audio_codec(data: &[u8]) -> Result<(u32, Option<u16>, bool)> {
        if data.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Empty data",
            ));
        }

        // Check for Vorbis
        if data.len() >= 7 && &data[1..7] == b"vorbis" {
            if data.len() >= 30 {
                let sample_rate = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
                let nominal_bitrate = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
                let max_bitrate = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
                let min_bitrate = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);

                let is_vbr = min_bitrate != max_bitrate || nominal_bitrate == 0;
                return Ok((sample_rate, Some(16), is_vbr));
            }
        }
        // Check for Opus
        else if data.len() >= 8 && &data[0..8] == b"OpusHead" {
            if data.len() >= 19 {
                let input_sample_rate =
                    u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
                return Ok((input_sample_rate, None, true));
            }
        }
        // Check for FLAC in Ogg container (starts with 0x7F + "FLAC")
        else if data.len() >= 5 && data[0] == 0x7F && &data[1..5] == b"FLAC" {
            if data.len() >= 13 + 34 {
                // Need enough bytes for FLAC metadata header + STREAMINFO
                // The FLAC metadata block starts after 13 bytes (0x7F + "FLAC" + 8 bytes)
                let metadata_header = &data[13..];

                // First byte: block type (0 for STREAMINFO) and flags
                let block_type = metadata_header[0] & 0x7F;
                if block_type != 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "First FLAC metadata block should be STREAMINFO",
                    ));
                }

                // Next 3 bytes: block length (big-endian)
                let block_length = u32::from_be_bytes([
                    0,
                    metadata_header[1],
                    metadata_header[2],
                    metadata_header[3],
                ]) as usize;

                if block_length != 34 || data.len() < 13 + 4 + 34 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid STREAMINFO block length",
                    ));
                }

                let streaminfo = &metadata_header[4..4 + 34];

                // Sample rate is 20 bits at bits 88-107 of STREAMINFO (bytes 10-12)
                let sample_rate =
                    u32::from_be_bytes([0, streaminfo[10], streaminfo[11], streaminfo[12]]) >> 4;

                // Bit depth is 5 bits at bits 108-112 (bits 4-0 of byte 13)
                let bits_per_sample =
                    ((streaminfo[12] & 0x01) << 4 | (streaminfo[13] >> 4)) as u16 + 1;

                // Total samples is 36 bits at bits 0-35 (bytes 0-4)
                let _total_samples = u64::from_be_bytes([
                    0,
                    0,
                    streaminfo[0],
                    streaminfo[1],
                    streaminfo[2],
                    streaminfo[3],
                    streaminfo[4],
                    streaminfo[5],
                ]) >> 4;

                // FLAC is vbr
                return Ok((sample_rate, Some(bits_per_sample), true));
            }
        }
        // Check for Speex
        else if data.len() >= 8 && &data[0..8] == b"Speex   " {
            if data.len() >= 68 {
                let sample_rate = u32::from_le_bytes([data[36], data[37], data[38], data[39]]);
                return Ok((sample_rate, None, true));
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Not an audio codec",
        ))
    }

    fn calculate_duration(file: &mut File, sample_rate: u32) -> Result<f64> {
        file.seek(SeekFrom::Start(0))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let mut last_granule = 0u64;
        let mut pos = 0;

        // Find last granule position
        while pos < buffer.len() {
            if let Some(next_pos) = Self::find_next_page(&buffer[pos..]) {
                pos += next_pos;
                if pos + 26 < buffer.len() {
                    let granule = u64::from_le_bytes([
                        buffer[pos + 6],
                        buffer[pos + 7],
                        buffer[pos + 8],
                        buffer[pos + 9],
                        buffer[pos + 10],
                        buffer[pos + 11],
                        buffer[pos + 12],
                        buffer[pos + 13],
                    ]);
                    if granule != u64::MAX && granule > 0 {
                        last_granule = granule;
                    }

                    // Skip to next page
                    if pos + 27 < buffer.len() {
                        let page_segments = buffer[pos + 26] as usize;
                        pos += 27 + page_segments;
                        if pos < buffer.len() {
                            let data_size: usize = buffer[pos - page_segments..pos]
                                .iter()
                                .map(|&x| x as usize)
                                .sum();
                            pos += data_size;
                        }
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Prevent division by zero
        if sample_rate == 0 {
            return Ok(0.0);
        }

        Ok(last_granule as f64 / sample_rate as f64)
    }

    fn find_next_page(buffer: &[u8]) -> Option<usize> {
        for i in 0..buffer.len() - 3 {
            if &buffer[i..i + 4] == b"OggS" {
                return Some(i);
            }
        }
        None
    }
}
