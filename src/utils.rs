use byte_unit::Byte;
use chrono::Local;
use crossterm::event::{Event, KeyModifiers};
use md5::{Digest, Md5};
use num_format::{Buffer, Locale, ToFormattedStr};
use ratatui::{
    style::Style,
    text::{Line, Span},
    widgets::{Block, Paragraph, Tabs},
};
use socket2::TcpKeepalive;

use crate::styles::{STYLE_DEFAULT_HIGHLIGHT_LOW_CONTRAST, STYLE_DEFAULT_LOW_CONTRAST};

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

pub(crate) fn key_events_into_paragraph_tabs<'a>(
    key_events: Vec<(Event, String)>,
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
    key_events.iter().for_each(|(event, hint)| {
        let text = if let Event::Key(key) = event {
            Span::from(if key.modifiers == KeyModifiers::NONE {
                format!(" {:?} = {} ", key.code, hint)
            } else {
                format!(
                    " {} + {:?} = {} ",
                    keymodifiers_to_string(key.modifiers),
                    key.code,
                    hint
                )
            })
        } else {
            unimplemented!()
        };
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
        } else if current_line.len() != 0 {
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

/// Gets the width of a character (in terms of ratatui screen units)
///
/// The width will either be 1 (utf8) or 2 (utf16 / unicode)
pub(crate) fn unicode_width(ch: char) -> u8 {
    if ch.len_utf8() == 1 {
        1
    } else {
        2
    }
}

/// Gets the width of each character (in terms of ratatui screen units) in a string and returns (width, char) pairs
pub(crate) fn unicode_char_lengths<S>(string: S) -> Vec<(u8, char)>
where
    S: Iterator<Item = char>,
{
    string.into_iter().map(|c| (unicode_width(c), c)).collect()
}

// pub(crate) fn concat_text<'a, T>(text: T, max_width: usize)
// where
//     T: Into<Text<'a>>,
// {
//     const GENERIC_STYLE: Style = Style::new();
//     let text: Text = text.into();
//     let mut previous_style: Style =  Style::new();
//     let mut current_line: Vec<StyledGrapheme> = vec![];
//     let mut lines: Vec<Line> = vec![];
//     let mut spans = vec![];
//     let mut graphemes: Vec<StyledGrapheme> = vec![];
//     let mut current_width: usize = 0;

//     for line in text.lines {
//         spans.extend(line.spans)
//     }

//     for span in &spans {
//         graphemes.extend(span.styled_graphemes(GENERIC_STYLE));
//     }

//     for grapheme in graphemes {
//         if previous_style != grapheme.style {
//             current_line.push(

//             )
//         }

//         if current_width + 1 <= max_width {
//             current_width += 1;
//             current_line.push(grapheme);
//         } else {
//             lines.push(Line::from(Span::from(current_line)))
//         }
//     }
// }

/// Splits a line (e.g. `String.chars()`) into chunks of (at most) `line_width` width (in terms of ratatui screen units)
pub(crate) fn into_unicode_chunks<C>(chars: C, line_width: u16) -> Vec<String>
where
    C: Iterator<Item = char>,
{
    let mut current_width = 0;
    let mut lines: Vec<String> = vec![];
    let mut current_line: String = String::new();

    for ch in chars {
        let width = unicode_width(ch) as u16;
        if width + current_width <= line_width {
            current_width += width;
            current_line.push(ch)
        } else {
            current_width = 0;
            lines.push(current_line);
            current_line = String::from(ch);
        }
    }

    if current_width != 0 {
        lines.push(current_line)
    }

    lines
}

pub(crate) fn num_as_str<N>(num: N) -> String
where
    N: ToFormattedStr,
{
    let mut buf = Buffer::default();
    buf.write_formatted(&num, &Locale::en);
    buf.to_string()
}

pub(crate) fn num_as_bytes(num: u64) -> String {
    format!(
        "{:.2}",
        Byte::from_u64(num).get_appropriate_unit(byte_unit::UnitType::Decimal)
    )
}
