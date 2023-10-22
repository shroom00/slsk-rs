use std::fmt::Display;

use chrono::{DateTime, TimeZone};
use crossterm::event::{KeyEvent, KeyModifiers};
use md5::{Digest, Md5};
use ratatui::{
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};
use socket2::TcpKeepalive;

pub(crate) fn latin1_to_string(s: &[u8]) -> String {
    s.iter().map(|&c| c as char).collect()
}

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

pub(crate) fn now_as_string<Tz>(datetime: DateTime<Tz>) -> String
where
    Tz: TimeZone,
    <Tz as TimeZone>::Offset: Display,
{
    format!("{}", datetime.format("%Y-%m-%d %H:%M:%S"))
}

pub(crate) fn keymodifiers_to_string(key: KeyModifiers) -> String {
    format!("{key:?}")
        .strip_prefix("KeyModifiers(")
        .unwrap()
        .strip_suffix(")")
        .unwrap()
        .replace("|", "+")
}

pub(crate) fn mask_string(input: &str) -> String {
    let mut masked = String::with_capacity(input.len());
    for _ in input.chars() {
        masked.push('*');
    }
    masked
}

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
fn keepalive_add_retries(ka: TcpKeepalive) -> TcpKeepalive {
    ka.with_retries(10)
}

pub(crate) const fn invert_style(style: Style) -> Style {
    match (style.fg, style.bg) {
        (Some(fg), Some(bg)) => Style::new().fg(bg).bg(fg),
        _ => unimplemented!(),
    }
}

pub(crate) fn key_events_into_paragraph_tabs<'a>(
    key_events: Vec<(KeyEvent, String)>,
    divider: Option<&'a str>,
    width: u16,
) -> (Paragraph<'a>, u16) {
    let width = width.max(4) - 4;
    let divider = match divider {
        Some(d) => d,
        None => "",
    };
    let divider_span = Span::from(divider);
    let mut tabs: Vec<Vec<Span>> = vec![];
    let mut current_line: Vec<Span> = vec![];
    let mut current_width = 0;
    let mut num_of_lines = 0;
    let mut current_hint_num: usize = 1;
    key_events.iter().for_each(|(key, hint)| {
        let text = Span::from(if key.modifiers == KeyModifiers::NONE {
            format!(" {:?} = {} ", key.code, hint)
        } else {
            format!(
                " {} + {:?} = {} ",
                keymodifiers_to_string(key.modifiers),
                key.code,
                hint
            )
        });
        // if room for text
        let t_width = text.width() as u16;
        if t_width + current_width < width {
            current_width += t_width;
            current_line.push(text);
            let d_width = divider_span.width() as u16;
            if d_width + current_width < width {
                current_width += d_width;
                current_line.push(divider_span.clone());
            }
            if current_hint_num == key_events.len() {
                if current_line[current_line.len() - 1].content.to_string() == divider {
                    current_line.pop();
                }
                tabs.push(current_line.clone());
                current_width = 0;
                num_of_lines += 1;
            }
        } else {
            if current_line[current_line.len() - 1].content.to_string() == divider {
                current_line.pop();
            }
            tabs.push(current_line.clone());
            current_line = vec![];
            current_width = 0;
            num_of_lines += 1
        }
        current_hint_num += 1;
    });
    let lines: Vec<Line<'_>> = tabs.into_iter().map(|line| Line::from(line)).collect();
    (Paragraph::new(lines), num_of_lines)
}
