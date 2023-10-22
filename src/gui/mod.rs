mod styles;
mod widgets;
mod windows;

use crate::{
    events::SLSKEvents,
    utils::key_events_into_paragraph_tabs,
};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Style, Styled, Stylize},
    symbols,
    widgets::{Block, Borders, Tabs},
    Frame, Terminal,
};
use std::{error::Error, io};
use tokio::sync::broadcast::{Receiver, Sender};

use self::{
    styles::*,
    windows::{login::LoginWindow, WidgetWithHints, Window},
};

macro_rules! make_window_enum {
    (
        $(
            $type:ident {$($lifetime:tt)*}
            {$($all_lifetimes:lifetime)+}
        )+
    ) => {
        #[derive(Clone)]
        enum WindowEnum<$($($lifetime)*,)+> {
            $(
                $type($type<$($lifetime)*>),
            )+
        }

        impl<$($($lifetime)*,)+> WindowEnum<$($($lifetime)*,)+> {
            fn get_title(&self) -> String {
                match self {
                    $(
                        WindowEnum::$type(window) => window.get_title(),
                    )+
                }
            }

            fn get_hints(&self) -> Vec<(KeyEvent, String)> {
                match self {
                    $(
                        WindowEnum::$type(window) => window.get_hints(),
                    )+
                }
            }

            fn perform_action(&mut self, focus_index: u8, key: KeyEvent, write_queue: &Sender<SLSKEvents>) {
                match self {
                        $(
                            WindowEnum::$type(ref mut window) => window.perform_action(focus_index, key, write_queue),
                        )+
                    }
            }

            fn number_of_widgets(&self) -> u8 {
                match self {
                    $(
                        WindowEnum::$type(window) => window.number_of_widgets(),
                    )+
                }
            }

            fn get_focused_index(&self) -> u8 {
                match self {
                    $(
                        WindowEnum::$type(window) => window.get_focused_index(),
                    )+
                }
            }

            fn set_focused_index(&mut self, index: u8) {
                match self {
                    $(
                        WindowEnum::$type(window) => window.set_focused_index(index),
                    )+
                }
            }
        }

        $(
            impl<$($all_lifetimes,)+> From<WindowEnum<$($all_lifetimes,)+>> for $type<$($lifetime,)+> {
                fn from(value: WindowEnum<$($all_lifetimes,)+>) -> Self {
                    match value {
                        WindowEnum::$type(window) => window,
                        _ => unimplemented!()
                    }
                }
            }
        )+
    };
}

make_window_enum!(
    LoginWindow {'a}
    {'a}
);

#[derive(Clone)]
struct App<'a> {
    windows: Vec<WindowEnum<'a>>,
    current_index: u8,
    select_index: u8,
    focused_widget: u8,
    hints: Vec<(KeyEvent, String)>,
}

impl<'a> App<'a> {
    fn render_current_window_on_frame<B: Backend>(&self, f: &mut Frame<'_, B>, area: Rect) {
        match self.get_current_window() {
            WindowEnum::LoginWindow(widget) => f.render_widget(widget.to_owned(), area),
        }
    }

    fn get_windows(&self) -> Vec<&WindowEnum> {
        self.windows.iter().map(|w| w).collect()
    }

    fn get_current_window(&self) -> &WindowEnum {
        self.get_windows()[self.current_index as usize]
    }

    fn get_window_count(&self) -> usize {
        self.get_windows().len()
    }
}

impl<'a> Default for App<'a> {
    fn default() -> App<'a> {
        App {
            windows: vec![
                WindowEnum::LoginWindow(LoginWindow::default()),
                WindowEnum::LoginWindow(LoginWindow::default()),
            ],
            current_index: 0,
            select_index: 0,
            focused_widget: 0,
            hints: vec![
                (
                    KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
                    String::from("Change window"),
                ),
                (
                    KeyEvent::new(KeyCode::Esc, KeyModifiers::SHIFT),
                    String::from("Quit"),
                ),
                (
                    KeyEvent::new(KeyCode::Tab, KeyModifiers::CONTROL),
                    String::from("Next window"),
                ),
                (
                    KeyEvent::new(KeyCode::Tab, KeyModifiers::CONTROL | KeyModifiers::SHIFT),
                    String::from("Previous window"),
                ),
                (
                    KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
                    String::from("Next widget"),
                ),
                (
                    KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT),
                    String::from("Previous widget"),
                ),
            ],
        }
    }
}

pub fn main(
    write_queue: Sender<SLSKEvents>,
    gui_queue: Receiver<SLSKEvents>,
) -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::default();

    run_app(&mut terminal, app, gui_queue, write_queue)?;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    println!("slsk-rs is exiting. Expect up to a few seconds with no response.");
    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    mut gui_queue: Receiver<SLSKEvents>,
    write_queue: Sender<SLSKEvents>,
) -> io::Result<()> {
    loop {
        let gui_event = gui_queue.try_recv().ok();
        match gui_event {
            Some(gui_event) => match gui_event {
                SLSKEvents::LoginResult(success, reason) => {
                    // This is hardcoded because we know the first window is the login window.
                    let mut login_window = LoginWindow::from(app.windows[0].clone());
                    let label: String;
                    let style: Style;
                    match success {
                        true => {
                            label = String::from("LOGGED IN");
                            style = STYLE_DISABLED_DEFAULT;
                            login_window.login_button.disable();
                        }
                        false => {
                            // Unwrapping is safe here because a failed login will always have a reason
                            label = String::from(format!("LOG IN FAILED: {}", reason.unwrap()));
                            style = STYLE_FAIL_DEFAULT;
                        }
                    };
                    login_window.login_button.set_label(label);
                    login_window.login_button = login_window.login_button.set_style(style);

                    // As we can't move the login window (as `LoginWindow`) out of `app.windows`,
                    // we clone the window and *then* convert it to `LoginWindow`.
                    // We alter the cloned window, then convert it back to the relevant enum variant
                    // and reassign the window in the app.
                    app.windows[0] = WindowEnum::LoginWindow(login_window);
                }

                SLSKEvents::Quit => return Ok(()),
                _ => (),
            },
            None => (),
        }
        terminal.draw(|f| ui(f, &app))?;

        let window_count = app.get_window_count();
        let window = &mut app.windows[app.current_index as usize];

        if let Event::Key(key) = event::read()? {
            match key.kind {
                event::KeyEventKind::Press => (),
                event::KeyEventKind::Repeat => (),
                // If Release isn't ignored, double key presses are registered (on Windows)
                event::KeyEventKind::Release => continue,
            };

            if key.modifiers == (KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Tab => app.select_index = (app.select_index + 1) % window_count as u8,
                    KeyCode::Enter => {
                        app.current_index = app.select_index;
                        app.focused_widget = window.get_focused_index()
                    }
                    _ => window.perform_action(app.focused_widget, key, &write_queue),
                }
            } else if key.modifiers == (KeyModifiers::SHIFT | KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::BackTab => {
                        if app.select_index == 0 {
                            app.select_index = (window_count - 1) as u8
                        } else {
                            app.select_index = (app.select_index - 1) % window_count as u8
                        }
                    }
                    _ => window.perform_action(app.focused_widget, key, &write_queue),
                }
            } else if key.modifiers == (KeyModifiers::SHIFT) {
                match key.code {
                    KeyCode::Esc => {
                        let _ = write_queue.send(SLSKEvents::Quit);
                        return Ok(());
                    }
                    KeyCode::BackTab => {
                        app.focused_widget = if app.focused_widget == 0 {
                            window.number_of_widgets() - 1
                        } else {
                            app.focused_widget - 1
                        };
                        window.set_focused_index(app.focused_widget);
                    }
                    _ => window.perform_action(app.focused_widget, key, &write_queue),
                }
            } else {
                match key.code {
                    KeyCode::Tab => {
                        app.focused_widget = (app.focused_widget + 1) % window.number_of_widgets();
                        window.set_focused_index(app.focused_widget);
                    }
                    _ => window.perform_action(app.focused_widget, key, &write_queue),
                }
            };
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &App) {
    let app_windows = app.get_windows();
    let current_window = app_windows[app.current_index as usize];

    let hints = app.hints.clone();
    let current_hints = current_window.get_hints();

    let divider = symbols::line::VERTICAL;

    let (current_hint_paragraph, current_hint_lines) =
        key_events_into_paragraph_tabs(current_hints, Some(divider), f.size().width);
    let current_hint_paragraph = current_hint_paragraph
        .alignment(ratatui::prelude::Alignment::Center)
        .set_style(STYLE_DEFAULT_LOW_CONTRAST);

    let (hint_paragraph, hint_lines) =
        key_events_into_paragraph_tabs(hints, Some(divider), f.size().width);
    let hint_paragraph = hint_paragraph
        .alignment(ratatui::prelude::Alignment::Center)
        .set_style(STYLE_DEFAULT_LOW_CONTRAST)
        .block(Block::default().borders(Borders::TOP));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(current_hint_lines.max(1)),
                // + 1 is to include the border
                Constraint::Length(hint_lines + 1),
            ]
            .as_ref(),
        )
        .split(f.size());

    let mut titles: Vec<String> = app_windows
        .iter()
        .map(|w| w.get_title().to_string())
        .collect();
    titles[app.current_index as usize] = titles[app.current_index as usize].to_ascii_uppercase();
    let titles = Tabs::new(titles)
        .block(Block::default())
        .style(STYLE_DEFAULT_LOW_CONTRAST)
        .bold()
        .highlight_style(STYLE_DEFAULT_HIGHLIGHT_LOW_CONTRAST)
        .not_bold()
        // .divider("")
        .select(app.select_index as usize);
    f.render_widget(titles, chunks[0]);

    app.render_current_window_on_frame(f, chunks[1]);

    f.render_widget(current_hint_paragraph, chunks[2]);
    f.render_widget(hint_paragraph, chunks[3]);
}
