use crate::utils::invert_style;
use ratatui::style::{Color, Style};

pub(crate) const COLOUR_DARK: Color = Color::Indexed(236);
pub(crate) const COLOUR_ACCENT: Color = Color::Green;
pub(crate) const COLOUR_FAIL_ACCENT: Color = Color::Red;
pub(crate) const COLOUR_DISABLED_ACCENT: Color = COLOUR_DARK;

pub(crate) const STYLE_DEFAULT: Style = Style::new().bg(Color::Black).fg(COLOUR_ACCENT);
pub(crate) const STYLE_DEFAULT_HIGHLIGHT: Style = invert_style(STYLE_DEFAULT);
pub(crate) const STYLE_DEFAULT_LOW_CONTRAST: Style = Style::new().bg(COLOUR_DARK).fg(COLOUR_ACCENT);
pub(crate) const STYLE_DEFAULT_HIGHLIGHT_LOW_CONTRAST: Style =
    invert_style(STYLE_DEFAULT_LOW_CONTRAST);

pub(crate) const STYLE_FAIL_DEFAULT: Style = Style::new().bg(Color::Black).fg(COLOUR_FAIL_ACCENT);
pub(crate) const STYLE_FAIL_DEFAULT_HIGHLIGHT: Style = invert_style(STYLE_FAIL_DEFAULT);
pub(crate) const STYLE_FAIL_DEFAULT_LOW_CONTRAST: Style =
    Style::new().bg(COLOUR_DARK).fg(COLOUR_FAIL_ACCENT);
pub(crate) const STYLE_FAIL_DEFAULT_HIGHLIGHT_LOW_CONTRAST: Style =
    invert_style(STYLE_FAIL_DEFAULT_LOW_CONTRAST);

pub(crate) const STYLE_DISABLED_DEFAULT: Style =
    Style::new().bg(Color::Black).fg(COLOUR_DISABLED_ACCENT);
pub(crate) const STYLE_DISABLED_DEFAULT_HIGHLIGHT: Style = invert_style(STYLE_DISABLED_DEFAULT);
pub(crate) const STYLE_DISABLED_DEFAULT_LOW_CONTRAST: Style =
    Style::new().bg(COLOUR_DARK).fg(COLOUR_DISABLED_ACCENT);
pub(crate) const STYLE_DISABLED_DEFAULT_HIGHLIGHT_LOW_CONTRAST: Style =
    invert_style(STYLE_DISABLED_DEFAULT_LOW_CONTRAST);
