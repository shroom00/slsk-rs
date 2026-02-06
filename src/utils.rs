use std::{fs::OpenOptions, io::{ErrorKind, Write}, path::Path};

use byte_unit::Byte;
use chrono::Local;
use crossterm::event::{Event, KeyModifiers};
use md5::{Digest, Md5};
use num_format::{Buffer, Locale, ToFormattedStr};
use ratatui::{
    layout::Constraint,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Paragraph, Tabs},
};
use socket2::TcpKeepalive;
use tokio::io::AsyncReadExt;

use crate::{SLSKExitCode, messages::MessageType, table::TableWidget, UnpackFromBytes};

#[allow(dead_code)]
pub(crate) fn latin1_to_string(s: &[u8]) -> String {
    s.iter().map(|&c| c as char).collect()
}

#[allow(dead_code)]
pub(crate) fn bytes_to_hex(bytes: &Vec<u8>) -> String {
    bytes
        .iter()
        .map(|byte| format!("{:02X} ", byte))
        .collect::<String>()
}

pub(crate) fn md5_digest(data: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(data);
    let result = hasher.finalize();

    let mut hash_str = String::new();
    for byte in result.iter() {
        hash_str.push_str(&format!("{:02x}", byte));
    }

    hash_str
}

pub(crate) fn now_as_string() -> String {
    format!("{}", Local::now().format("%Y-%m-%d %H:%M:%S"))
}

pub(crate) fn keymodifiers_to_string(key: KeyModifiers) -> String {
    format!("{key:?}")
        .strip_prefix("KeyModifiers(")
        .unwrap()
        .strip_suffix(")")
        .unwrap()
        .replace("|", "+")
}

#[cfg(any(
    target_os = "openbsd",
    target_os = "redox",
    target_os = "solaris",
    target_os = "windows",
    target_os = "nto",
    target_os = "espidf",
))]
pub(crate) fn keepalive_add_retries(ka: TcpKeepalive) -> TcpKeepalive {
    ka
}

#[cfg(not(any(
    target_os = "openbsd",
    target_os = "redox",
    target_os = "solaris",
    target_os = "windows",
    target_os = "nto",
    target_os = "espidf",
)))]
pub(crate) fn keepalive_add_retries(ka: TcpKeepalive) -> TcpKeepalive {
    ka.with_retries(10)
}

pub(crate) fn key_events_into_paragraph_tabs<'a>(
    key_events: Vec<(Event, String)>,
    divider: Option<&'a str>,
    width: u16,
) -> (Paragraph<'a>, u16) {
    let min_width = 4;
    let width = width.max(min_width) - min_width;
    let divider = divider.unwrap_or("");
    let divider_span = Span::from(divider);

    let mut tabs: Vec<Vec<Span>> = Vec::new();
    let mut current_line: Vec<Span> = Vec::new();
    let mut current_width = 0;
    let mut num_of_lines = 1;

    for (i, (event, hint)) in key_events.iter().enumerate() {
        let text = if let Event::Key(key) = event {
            if key.modifiers == KeyModifiers::NONE {
                Span::from(format!(" {:?} = {} ", key.code, hint))
            } else {
                Span::from(format!(
                    " {} + {:?} = {} ",
                    keymodifiers_to_string(key.modifiers),
                    key.code,
                    hint
                ))
            }
        } else {
            unimplemented!()
        };

        let text_width = text.width() as u16;
        let divider_width = divider_span.width() as u16;

        if current_width + text_width > width {
            if let Some(last) = current_line.last() {
                if last.content == divider {
                    current_line.pop();
                }
            }

            tabs.push(current_line);
            current_line = Vec::new();
            current_width = 0;
            num_of_lines += 1;
        }

        current_line.push(text);
        current_width += text_width;

        if i < key_events.len() - 1 && current_width + divider_width <= width {
            current_line.push(divider_span.clone());
            current_width += divider_width;
        }
    }

    if !current_line.is_empty() {
        if let Some(last) = current_line.last() {
            if last.content == divider {
                current_line.pop();
            }
        }
        tabs.push(current_line);
    }

    let lines = tabs.into_iter().map(Line::from).collect::<Vec<_>>();
    (Paragraph::new(lines), num_of_lines)
}

pub(crate) fn vec_string_to_tabs<'a>(
    mut titles: Vec<String>,
    style: Style,
    highlight_style: Style,
    select_index: usize,
    current_index: usize,
) -> Tabs<'a> {
    if titles.len() > 0 {
        titles[current_index] = titles[current_index].to_ascii_uppercase();
    }
    let tabs = Tabs::new(titles.clone())
        .block(Block::default())
        .style(style)
        .highlight_style(highlight_style);
    if titles.len() > 0 {
        tabs.select(select_index)
    } else {
        tabs
    }
}

pub(crate) fn num_as_str<N>(num: N) -> String
where
    N: ToFormattedStr,
{
    let mut buf = Buffer::default();
    buf.write_formatted(&num, &Locale::en);
    buf.to_string()
}

/// Results will be 10 characters at most.
/// Outputs in the form 123.45 MB etc.
pub(crate) fn num_as_bytes(num: u64) -> String {
    format!(
        "{:.2}",
        Byte::from_u64(num).get_appropriate_unit(byte_unit::UnitType::Decimal)
    )
}

pub(crate) fn default_results_table<'a>() -> TableWidget<'a> {
    TableWidget::new(
        vec![
            String::from("User"),
            String::from("Speed"),
            String::from("Queue"),
            String::from("Folder"),
            String::from("Filename"),
            String::from("Size"),
        ],
        Vec::new(),
        Some(3..5),
        Some(vec![
            Constraint::Max(30), // username
            Constraint::Max(10), // average speed
            Constraint::Max(6),  // queue length
            Constraint::Fill(1), // folder
            Constraint::Fill(2), // filename
            Constraint::Max(10), // filesize
        ]),
    )
}

/// Adds newline to text
pub(crate) fn log<T: Into<String>>(text: T) {
    if crate::LOGGING_ENABLED {
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open("MY.log")
            .unwrap();
        f.write_all(format!("{}: {}\n", now_as_string(), &text.into()).as_bytes())
            .unwrap();
    }
}

pub(crate) fn file_is_hidden(path: &Path) -> bool {
    if path.file_name().unwrap().to_string_lossy().starts_with(".") {
        return true;
    }
    #[cfg(target_os = "windows")]
    {
        use std::fs::metadata;

        match metadata(path) {
            Ok(f) => {
                // thanks to https://users.rust-lang.org/t/portable-way-to-check-if-a-file-is-hidden/106783/2 :)
                const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
                let attrs = std::os::windows::fs::MetadataExt::file_attributes(&f);
                return attrs & FILE_ATTRIBUTE_HIDDEN != 0;
            }
            Err(_) => (),
        };
    }
    false
}

// TODO: Make this use chunks + change usage to account for this. (?)
pub(crate) async fn get_code_and_bytes_from_readable<R>(
    reader: &mut R,
    message_type: MessageType,
) -> Result<(MessageType, Vec<u8>), SLSKExitCode>
where
    R: AsyncReadExt + Unpin,
{
    let mut length: [u8; 4] = [0, 0, 0, 0];
    match reader.read_exact(&mut length).await {
        Ok(_) => (),
        Err(e) => return Err(SLSKExitCode::IoError(e)),
    }
    let length = u32::from_le_bytes(length);
    let mut bytes: Vec<u8> = vec![0; length as usize];

    match reader.read_exact(&mut bytes).await {
        Ok(_) => (),
        Err(e) => return Err(SLSKExitCode::IoError(e)),
    }
    Ok(match message_type {
        MessageType::Server(_) => (
            MessageType::Server(match <u32>::unpack_from_bytes(&mut bytes) {
                Some(n) => n,
                None => return Err(SLSKExitCode::IoError(std::io::Error::from(ErrorKind::InvalidData))),
            }),
            bytes,
        ),
        MessageType::PeerInit(_) => (
            MessageType::PeerInit(match <u8>::unpack_from_bytes(&mut bytes) {
                Some(n) => n,
                None => return Err(SLSKExitCode::IoError(std::io::Error::from(ErrorKind::InvalidData))),
            }),
            bytes,
        ),
        MessageType::Peer(_) => (
            MessageType::Peer(match <u32>::unpack_from_bytes(&mut bytes) {
                Some(n) => n,
                None => return Err(SLSKExitCode::IoError(std::io::Error::from(ErrorKind::InvalidData))),
            }),
            bytes,
        ),
        MessageType::File => unimplemented!(),
        MessageType::Distributed(_) => (
            MessageType::Distributed(match <u8>::unpack_from_bytes(&mut bytes) {
                Some(n) => n,
                None => return Err(SLSKExitCode::IoError(std::io::Error::from(ErrorKind::InvalidData))),
            }),
            bytes,
        ),
    })
}
