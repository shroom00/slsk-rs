use ratatui::style::{Color, Style, Modifier};

pub(crate) const COLOUR_DARK: Color = Color::Indexed(236);
pub(crate) const COLOUR_ACCENT: Color = Color::Green;
pub(crate) const COLOUR_FAIL_ACCENT: Color = Color::Red;
pub(crate) const COLOUR_DISABLED_ACCENT: Color = COLOUR_DARK;

pub(crate) const STYLE_DEFAULT: Style = Style::new().bg(Color::Black).fg(COLOUR_ACCENT);
pub(crate) const STYLE_DEFAULT_HIGHLIGHT: Style = STYLE_DEFAULT.add_modifier(Modifier::REVERSED);
pub(crate) const STYLE_DEFAULT_LOW_CONTRAST: Style = Style::new().bg(COLOUR_DARK).fg(COLOUR_ACCENT);
pub(crate) const STYLE_DEFAULT_HIGHLIGHT_LOW_CONTRAST: Style =
    STYLE_DEFAULT_LOW_CONTRAST.add_modifier(Modifier::REVERSED);

pub(crate) const STYLE_FAIL_DEFAULT: Style = Style::new().bg(Color::Black).fg(COLOUR_FAIL_ACCENT);
pub(crate) const STYLE_FAIL_DEFAULT_HIGHLIGHT: Style = STYLE_FAIL_DEFAULT.add_modifier(Modifier::REVERSED);
pub(crate) const STYLE_FAIL_DEFAULT_LOW_CONTRAST: Style =
    Style::new().bg(COLOUR_DARK).fg(COLOUR_FAIL_ACCENT);
pub(crate) const STYLE_FAIL_DEFAULT_HIGHLIGHT_LOW_CONTRAST: Style =
    STYLE_FAIL_DEFAULT_LOW_CONTRAST.add_modifier(Modifier::REVERSED);

pub(crate) const STYLE_DISABLED_DEFAULT: Style =
    Style::new().bg(Color::Black).fg(COLOUR_DISABLED_ACCENT);
pub(crate) const STYLE_DISABLED_DEFAULT_HIGHLIGHT: Style = STYLE_DISABLED_DEFAULT.add_modifier(Modifier::REVERSED);
pub(crate) const STYLE_DISABLED_DEFAULT_LOW_CONTRAST: Style =
    Style::new().bg(COLOUR_DARK).fg(COLOUR_DISABLED_ACCENT);
pub(crate) const STYLE_DISABLED_DEFAULT_HIGHLIGHT_LOW_CONTRAST: Style =
    STYLE_DISABLED_DEFAULT_LOW_CONTRAST.add_modifier(Modifier::REVERSED);
