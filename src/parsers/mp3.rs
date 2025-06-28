use std::fs::File;
use std::io::{Read, Result, Seek, SeekFrom};
use std::path::Path;

use super::{AudioMetadata, Bitrate};

pub(crate) struct MpParser {
    bitrate: Bitrate,
    duration: f64,
    vbr: bool,
    sample_rate: u32,
}

#[derive(Clone, Copy, Debug)]
enum MpegVersion {
    V2_5,
    V2,
    V1,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum MpegLayer {
    Layer1,
    Layer2,
    Layer3,
}

impl MpegLayer {
    fn samples_per_frame(&self) -> f64 {
        match self {
            MpegLayer::Layer1 => 384.0,
            _ => 1152.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct FrameHeader {
    _version: MpegVersion,
    layer: MpegLayer,
    bitrate: u16, // Store as kbps, 0 means free bitrate
    sample_rate: u32,
    padding_bit: bool,
}

impl FrameHeader {
    /// Calculates the frame length in bytes (compressed size)
    fn frame_length(&self) -> Option<u32> {
        if self.bitrate == 0 {
            return None; // Free bitrate - cannot calculate
        }

        let bitrate_bps = self.bitrate as u32 * 1000;

        let frame_length = match self.layer {
            MpegLayer::Layer1 => {
                // Layer I: (12 * bitrate / sample_rate + padding) * 4
                (12 * bitrate_bps / self.sample_rate + self.padding_bit as u32) * 4
            }
            _ => {
                // Layer II & III: 144 * bitrate / sample_rate + padding
                144 * bitrate_bps / self.sample_rate + self.padding_bit as u32
            }
        };

        Some(frame_length)
    }
}

#[derive(Debug)]
struct VbrHeader {
    frames: Option<u32>,
    bytes: Option<u32>,
}

impl VbrHeader {
    fn parse(file: &mut File) -> Result<Option<Self>> {
        // Simple approach: look for Xing/Info at common offsets
        let offsets = [0, 32, 36]; // Most common offsets for Layer III

        for &offset in &offsets {
            if file.seek(SeekFrom::Current(offset)).is_err() {
                continue;
            }

            let mut magic = [0u8; 4];
            if file.read_exact(&mut magic).is_err() {
                file.seek(SeekFrom::Current(-offset))?;
                continue;
            }

            if magic == *b"Xing" || magic == *b"Info" {
                // Read flags
                let mut flags_buf = [0u8; 4];
                if file.read_exact(&mut flags_buf).is_err() {
                    file.seek(SeekFrom::Current(-offset - 4))?;
                    continue;
                }
                let flags = u32::from_be_bytes(flags_buf);

                // Read frame count if present
                let frames = if flags & 0x01 != 0 {
                    let mut buf = [0u8; 4];
                    file.read_exact(&mut buf)
                        .ok()
                        .map(|_| u32::from_be_bytes(buf))
                } else {
                    None
                };

                // Read byte count if present
                let bytes = if flags & 0x02 != 0 {
                    let mut buf = [0u8; 4];
                    file.read_exact(&mut buf)
                        .ok()
                        .map(|_| u32::from_be_bytes(buf))
                } else {
                    None
                };

                return Ok(Some(VbrHeader { frames, bytes }));
            }

            // Reset position for next attempt
            file.seek(SeekFrom::Current(-offset - 4))?;
        }

        Ok(None)
    }
}

impl MpParser {
    const MAX_RESYNC_BYTES: usize = 8192; // Limit search to prevent hanging

    fn resync_to_frame(file: &mut File) -> Result<bool> {
        let mut buf = [0u8; 1];
        let mut bytes_searched = 0;

        while bytes_searched < Self::MAX_RESYNC_BYTES {
            if file.read_exact(&mut buf).is_err() {
                return Ok(false); // EOF
            }
            bytes_searched += 1;

            if buf[0] == 0xFF {
                let mut next_byte = [0u8; 1];
                if file.read_exact(&mut next_byte).is_err() {
                    return Ok(false); // EOF
                }

                if (next_byte[0] & 0xE0) == 0xE0 {
                    // Found valid sync - rewind to start of frame
                    file.seek(SeekFrom::Current(-2))?;
                    return Ok(true);
                }
                // Not a valid sync, continue (next_byte[0] becomes the new buf[0])
                buf[0] = next_byte[0];
            }
        }

        Ok(false) // No sync found within limit
    }

    fn detect_vbr_by_sampling(file: &mut File) -> Result<bool> {
        let start_pos = file.stream_position()?;
        let mut frame_sizes = Vec::new();
        let mut frames_to_check = 5; // Sample first 5 frames
        let frames_to_skip = frames_to_check - 2;

        while frames_to_check > 0 {
            let mut header_buf = [0u8; 4];
            if file.read_exact(&mut header_buf).is_err() {
                break; // EOF
            }

            match Self::read_frame_header(header_buf) {
                Ok(header) => {
                    if let Some(frame_len) = header.frame_length() {
                        // Sanity check frame length
                        if frame_len > 4096 || frame_len < 21 {
                            break;
                        }

                        if frames_to_check < frames_to_skip {
                            frame_sizes.push(frame_len);
                        }
                        
                        // Skip to next frame
                        if file.seek(SeekFrom::Current(frame_len as i64 - 4)).is_err() {
                            break;
                        }

                        frames_to_check -= 1;
                    } else {
                        break; // Free bitrate or invalid
                    }
                }
                Err(_) => {
                    // Try to resync, but limit attempts
                    if !Self::resync_to_frame(file)? {
                        break;
                    }
                }
            }
        }

        // Reset position
        file.seek(SeekFrom::Start(start_pos))?;

        // Consider VBR if we found varying frame sizes
        if frame_sizes.len() >= 2 {
            let first_size = frame_sizes[0];
            Ok(frame_sizes.iter().any(|&size| size != first_size))
        } else {
            Ok(false) // Not enough data or uniform sizes
        }
    }

    fn read_frame_header(buffer: [u8; 4]) -> Result<FrameHeader> {
        // Check frame sync (11 bits of 1)
        let frame_sync = (buffer[0] as u16) << 3 | (buffer[1] as u16) >> 5;
        if frame_sync != 0b11111111111 {
            return Err(std::io::Error::other("Invalid Frame Sync"));
        }

        let version = match (buffer[1] >> 3) & 0b11 {
            0 => MpegVersion::V2_5,
            2 => MpegVersion::V2,
            3 => MpegVersion::V1,
            _ => return Err(std::io::Error::other("Invalid MPEG Version")),
        };

        let layer = match (buffer[1] >> 1) & 0b11 {
            1 => MpegLayer::Layer3,
            2 => MpegLayer::Layer2,
            3 => MpegLayer::Layer1,
            _ => return Err(std::io::Error::other("Invalid MPEG Layer")),
        };

        let bitrate_index = (buffer[2] >> 4) & 0b1111;
        if bitrate_index == 15 {
            return Err(std::io::Error::other("Invalid Bitrate Index"));
        }

        let bitrate = if bitrate_index == 0 {
            0 // Free bitrate
        } else {
            match (version, layer, bitrate_index as usize) {
                (MpegVersion::V1, MpegLayer::Layer1, idx) => 32 * idx as u16,
                (MpegVersion::V1, MpegLayer::Layer2, idx) => [
                    0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
                ][idx],
                (MpegVersion::V1, MpegLayer::Layer3, idx) => [
                    0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
                ][idx],
                (_, MpegLayer::Layer1, idx) => [
                    0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
                ][idx],
                (_, _, idx) => [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160][idx],
            }
        };

        let sample_rate = match (version, (buffer[2] >> 2) & 0b11) {
            (_, 3) => return Err(std::io::Error::other("Invalid Sample Rate")),
            (MpegVersion::V1, 0) => 44100,
            (MpegVersion::V1, 1) => 48000,
            (MpegVersion::V1, 2) => 32000,
            (MpegVersion::V2, 0) => 22050,
            (MpegVersion::V2, 1) => 24000,
            (MpegVersion::V2, 2) => 16000,
            (MpegVersion::V2_5, 0) => 11025,
            (MpegVersion::V2_5, 1) => 12000,
            (MpegVersion::V2_5, 2) => 8000,
            _ => return Err(std::io::Error::other("Invalid Sample Rate")),
        };

        let padding_bit = (buffer[2] >> 1) & 1 == 1; // Fixed: should be == 1

        Ok(FrameHeader {
            _version: version,
            layer,
            bitrate,
            sample_rate,
            padding_bit,
        })
    }
}

impl AudioMetadata for MpParser {
    fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = File::open(path)?;

        // Skip ID3v2 tag if present
        let mut buffer = [0; 4];
        file.read_exact(&mut buffer)?;
        if buffer[..3] == *b"ID3" {
            // Read tag size
            file.seek(SeekFrom::Current(2))?; // Skip version bytes
            file.read_exact(&mut buffer)?;

            // ID3v2 uses synchsafe integers (7 bits per byte)
            let size = ((buffer[0] & 0x7F) as u32) << 21
                | ((buffer[1] & 0x7F) as u32) << 14
                | ((buffer[2] & 0x7F) as u32) << 7
                | (buffer[3] & 0x7F) as u32;

            file.seek(SeekFrom::Current(size as i64))?;
            file.read_exact(&mut buffer)?;
        }

        // Parse first frame header
        let first_header = Self::read_frame_header(buffer)?;
        let sample_rate = first_header.sample_rate;

        // Look for VBR header (only for Layer III)
        let vbr_header = if first_header.layer == MpegLayer::Layer3 {
            VbrHeader::parse(&mut file)?
        } else {
            None
        };

        let (duration, bitrate, is_vbr) = if let Some(vbr) = vbr_header {
            // VBR file with Xing/Info header
            match (vbr.frames, vbr.bytes) {
                (Some(frame_count), Some(byte_count)) => {
                    let duration_sec = frame_count as f64 * first_header.layer.samples_per_frame()
                        / sample_rate as f64;
                    let avg_bitrate = ((byte_count * 8) as f64 / duration_sec / 1000.0) as u16;

                    // Quick check if actually VBR
                    let is_truly_vbr = Self::detect_vbr_by_sampling(&mut file)?;

                    (duration_sec, avg_bitrate, is_truly_vbr)
                }
                _ => {
                    // Fallback: estimate from file size
                    let file_size = file.metadata()?.len();
                    let audio_start = file.stream_position()?;
                    let audio_size = file_size.saturating_sub(audio_start);

                    // Rough estimate assuming average 128 kbps
                    let duration_sec = (audio_size as f64 * 8.0) / (128.0 * 1000.0);
                    (duration_sec, 128, true)
                }
            }
        } else if first_header.bitrate == 0 {
            // Free bitrate - estimate from file size and assume CBR
            let file_size = file.metadata()?.len();
            let audio_start = file.stream_position()?;
            let audio_size = file_size.saturating_sub(audio_start);

            // Rough duration estimate (assume 128 kbps)
            let duration_sec = (audio_size as f64 * 8.0) / (128.0 * 1000.0);
            (duration_sec, 128, false)
        } else {
            // Standard CBR
            let frame_size = first_header.frame_length().unwrap_or(417) as u64; // fallback size
            let file_size = file.metadata()?.len();
            let audio_start = file.stream_position()?;
            let audio_size = file_size.saturating_sub(audio_start);

            let total_frames = audio_size / frame_size;
            let duration_sec =
                total_frames as f64 * first_header.layer.samples_per_frame() / sample_rate as f64;

            (duration_sec, first_header.bitrate, false)
        };

        let bitrate_enum = if is_vbr {
            Bitrate::Variable(bitrate)
        } else {
            Bitrate::Constant(bitrate)
        };

        Ok(Self {
            bitrate: bitrate_enum,
            duration: duration.max(0.1), // Ensure positive duration
            vbr: is_vbr,
            sample_rate,
        })
    }

    fn bitrate(&self) -> Bitrate {
        self.bitrate
    }

    fn duration(&self) -> f64 {
        self.duration
    }

    fn is_vbr(&self) -> bool {
        self.vbr
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn bit_depth(&self) -> Option<u16> {
        None // MP3 doesn't have a fixed bit depth concept
    }
}
