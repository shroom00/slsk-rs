use std::fs::File;
use std::io::{Read, Result, Seek};
use std::path::Path;

use super::{AudioMetadata, Bitrate};

pub(crate) struct AiffParser {
    sample_rate: u32,
    bit_depth: Option<u16>,
    duration: f64,
    bitrate: Bitrate,
    _is_compressed: bool,
    _channels: u16,
}

impl AiffParser {
    fn read_u32_be(file: &mut File) -> Result<u32> {
        let mut buf = [0u8; 4];
        file.read_exact(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }

    fn read_u16_be(file: &mut File) -> Result<u16> {
        let mut buf = [0u8; 2];
        file.read_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }

    fn read_chunk_header(file: &mut File) -> Result<([u8; 4], u32)> {
        let mut chunk_id = [0u8; 4];
        file.read_exact(&mut chunk_id)?;
        let chunk_size = Self::read_u32_be(file)?;
        Ok((chunk_id, chunk_size))
    }

    fn read_ieee754_extended(file: &mut File) -> Result<f64> {
        let mut buf = [0u8; 10];
        file.read_exact(&mut buf)?;

        // IEEE 754 80-bit extended precision format
        let sign = (buf[0] & 0x80) != 0;
        let exponent = (((buf[0] & 0x7F) as u16) << 8) | (buf[1] as u16);

        // Read the 64-bit mantissa correctly
        let mantissa_high = u32::from_be_bytes([buf[2], buf[3], buf[4], buf[5]]);
        let mantissa_low = u32::from_be_bytes([buf[6], buf[7], buf[8], buf[9]]);
        let mantissa = ((mantissa_high as u64) << 32) | (mantissa_low as u64);

        if exponent == 0 && mantissa == 0 {
            return Ok(0.0);
        }

        if exponent == 0x7FFF {
            return Ok(if sign {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            });
        }

        // Convert to f64 - IEEE 754 extended uses bias of 16383
        let result = if exponent == 0 {
            // Denormalized number
            (mantissa as f64) * 2.0_f64.powi(-16382 - 63)
        } else {
            // Normalized number - the leading 1 bit is implicit in extended precision
            let exp = (exponent as i32) - 16383 - 63;
            (mantissa as f64) * 2.0_f64.powi(exp)
        };

        Ok(if sign { -result } else { result })
    }

    fn parse_comm_chunk(
        file: &mut File,
        chunk_size: u32,
        is_aifc: bool,
    ) -> Result<(u16, u32, u16, u32, Option<String>)> {
        let _channels = Self::read_u16_be(file)?;
        let num_sample_frames = Self::read_u32_be(file)?;
        let sample_size = Self::read_u16_be(file)?;
        let sample_rate = Self::read_ieee754_extended(file)? as u32;

        let compression_type = if is_aifc && chunk_size > 18 {
            let mut comp_type = [0u8; 4];
            file.read_exact(&mut comp_type)?;

            // Read compression name (Pascal string)
            let mut name_len = [0u8; 1];
            file.read_exact(&mut name_len)?;
            let name_len = name_len[0] as usize;

            if name_len > 0 {
                let mut name_bytes = vec![0u8; name_len];
                file.read_exact(&mut name_bytes)?;
            }

            // Skip padding byte if name length is even (Pascal string padding)
            if name_len % 2 == 0 {
                let _ = file.seek_relative(1);
            }

            Some(
                String::from_utf8_lossy(&comp_type)
                    .trim_end_matches('\0')
                    .to_string(),
            )
        } else {
            None
        };

        Ok((
            _channels,
            num_sample_frames,
            sample_size,
            sample_rate,
            compression_type,
        ))
    }

    fn is_compressed_format(compression_type: &str) -> bool {
        match compression_type {
            "NONE" | "twos" | "sowt" => false,          // Uncompressed PCM
            "fl32" | "fl64" | "FL32" | "FL64" => false, // Uncompressed floating point
            "alaw" | "ulaw" | "ALAW" | "ULAW" => true,  // Compressed (but CBR)
            "ima4" | "IMA4" => true,                    // IMA ADPCM (CBR)
            "MAC3" | "MAC6" => true,                    // MACE compression (CBR)
            "sdx2" | "SDX2" => true,                    // SDX2 compression (2:1)
            "G721" | "g721" => true,                    // G.721 ADPCM (2:1)
            "G722" | "g722" => true,                    // G.722 ADPCM
            "G723" | "g723" => true,                    // G.723 ADPCM
            "ms\x00\x02" => true,                       // MS ADPCM (2:1)
            _ => true,                                  // Assume compressed if unknown
        }
    }

    fn get_compression_ratio(compression_type: &str) -> f64 {
        match compression_type {
            "NONE" | "twos" | "sowt" => 1.0,          // Uncompressed PCM
            "fl32" | "fl64" | "FL32" | "FL64" => 1.0, // Uncompressed floating point
            "alaw" | "ulaw" | "ALAW" | "ULAW" => 2.0, // 8-bit compressed from 16-bit (2:1)
            "ima4" | "IMA4" => 4.0,                   // IMA ADPCM (4:1 compression)
            "sdx2" | "SDX2" => 2.0,                   // SDX2 compression (2:1)
            "G721" | "g721" => 2.0,                   // G.721 ADPCM (2:1)
            "G722" | "g722" => 2.0,                   // G.722 ADPCM (2:1)
            "G723" | "g723" => 3.0, // G.723 ADPCM (3:1 or 5:1, use 3:1 as conservative)
            "ms\x00\x02" => 2.0,    // MS ADPCM (approximately 2:1)
            "MAC3" => 3.0,          // MACE 3:1
            "MAC6" => 6.0,          // MACE 6:1
            _ => 2.0,               // Conservative estimate for unknown formats
        }
    }

    fn skip_chunk_with_padding(file: &mut File, chunk_size: u32) -> Result<()> {
        file.seek_relative(chunk_size as i64)?;
        // AIFF chunks are padded to even byte boundaries
        if chunk_size % 2 == 1 {
            file.seek_relative(1)?;
        }
        Ok(())
    }
}

impl AudioMetadata for AiffParser {
    fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(path)?;

        // Read FORM chunk header
        let mut form_header = [0u8; 4];
        file.read_exact(&mut form_header)?;
        if &form_header != b"FORM" {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Not a valid AIFF/AIFC file - missing FORM header",
            ));
        }

        let _form_size = Self::read_u32_be(&mut file)?;

        // Read form type (AIFF or AIFC)
        let mut form_type = [0u8; 4];
        file.read_exact(&mut form_type)?;
        let is_aifc = match &form_type {
            b"AIFF" => false,
            b"AIFC" => true,
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Not a valid AIFF/AIFC file - invalid form type",
                ))
            }
        };

        let mut sample_rate = 0u32;
        let mut bit_depth = None;
        let mut num_sample_frames = 0u32;
        let mut num_channels = 0u16;
        let mut _is_compressed = false;
        let mut is_vbr = false;
        let mut compression_ratio = 1.0;

        // Read chunks
        while let Ok((chunk_id, chunk_size)) = Self::read_chunk_header(&mut file) {
            match &chunk_id {
                b"COMM" => {
                    let (channels, frames, sample_size, parsed_sample_rate, compression_type) =
                        Self::parse_comm_chunk(&mut file, chunk_size, is_aifc)?;

                    num_channels = channels;
                    num_sample_frames = frames;
                    sample_rate = parsed_sample_rate;
                    bit_depth = Some(sample_size);

                    if let Some(comp_type) = compression_type {
                        _is_compressed = Self::is_compressed_format(&comp_type);
                        // assume AIFF is CBR
                        is_vbr = false;
                        compression_ratio = Self::get_compression_ratio(&comp_type);
                    }
                }
                b"SSND" => {
                    // Sound data chunk - skip it with proper padding
                    Self::skip_chunk_with_padding(&mut file, chunk_size)?;
                }
                _ => {
                    // Skip unknown chunks with proper padding
                    Self::skip_chunk_with_padding(&mut file, chunk_size)?;
                }
            }
        }

        if sample_rate == 0 || bit_depth.is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Missing required COMM chunk",
            ));
        }

        // Calculate duration more carefully
        let duration = if sample_rate > 0 && num_sample_frames > 0 {
            num_sample_frames as f64 / sample_rate as f64
        } else {
            0.0
        };

        // Calculate bitrate correctly
        let bitrate = if let Some(depth) = bit_depth {
            if sample_rate > 0 && num_channels > 0 && depth > 0 {
                // Calculate uncompressed bits per second
                let uncompressed_bps = sample_rate as u64 * num_channels as u64 * depth as u64;

                // Apply compression ratio to get actual bitrate
                let actual_bps = (uncompressed_bps as f64 / compression_ratio) as u64;

                // Convert to kbps using 1000 (audio industry standard)
                let kbps = (actual_bps / 1000) as u16;

                if is_vbr {
                    Bitrate::Variable(kbps)
                } else {
                    Bitrate::Constant(kbps)
                }
            } else {
                Bitrate::Constant(0)
            }
        } else {
            Bitrate::Constant(0)
        };

        Ok(Self {
            sample_rate,
            bit_depth,
            duration,
            bitrate,
            _is_compressed,
            _channels: num_channels,
        })
    }

    fn bitrate(&self) -> Bitrate {
        self.bitrate
    }

    fn duration(&self) -> f64 {
        self.duration
    }

    fn is_vbr(&self) -> bool {
        match self.bitrate {
            Bitrate::Variable(_) => true,
            Bitrate::Constant(_) => false,
        }
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn bit_depth(&self) -> Option<u16> {
        self.bit_depth
    }
}
