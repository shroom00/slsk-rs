use std::{io::Result, ops::Deref, path::Path};

pub(crate) use self::{
    aiff::AiffParser, flac::FlacParser, mp3::MpParser, ogg::OggParser, wav::WavParser,
};

pub(crate) mod aiff;
pub(crate) mod flac;
pub(crate) mod mp3;
pub(crate) mod ogg;
pub(crate) mod wav;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Bitrate {
    Constant(u16),
    Variable(u16),
}

impl Deref for Bitrate {
    type Target = u16;

    fn deref(&self) -> &Self::Target {
        match self {
            Bitrate::Constant(u) => u,
            Bitrate::Variable(u) => u,
        }
    }
}

pub(crate) trait AudioMetadata: Send {
    fn new<P: AsRef<Path>>(path: P) -> Result<Self>
    where
        Self: Sized;
    fn bitrate(&self) -> Bitrate;
    fn duration(&self) -> f64;
    fn is_vbr(&self) -> bool;
    fn sample_rate(&self) -> u32;
    fn bit_depth(&self) -> Option<u16>;
}

pub(crate) fn parse(path: &Path) -> Option<Result<Box<dyn AudioMetadata>>> {
    let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase())?;
    Some(match extension.as_str() {
        "mp3" | "mp2" | "mp1" => {
            MpParser::new(path).map(|metadata| Box::new(metadata) as Box<dyn AudioMetadata>)
        }
        "flac" => {
            FlacParser::new(path).map(|metadata| Box::new(metadata) as Box<dyn AudioMetadata>)
        }
        "wav" => WavParser::new(path).map(|metadata| Box::new(metadata) as Box<dyn AudioMetadata>),
        "opus" | "ogg" | "oga" => {
            OggParser::new(path).map(|metadata| Box::new(metadata) as Box<dyn AudioMetadata>)
        }
        "aiff" | "aifc" | "aif" => {
            AiffParser::new(path).map(|metadata| Box::new(metadata) as Box<dyn AudioMetadata>)
        }
        _ => return None,
    })
}
