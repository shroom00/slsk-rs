pub(crate) mod widgets;
mod windows;
use crate::styles::STYLE_DEFAULT;
use crate::utils::now_as_string;
use crate::{DownloadStatus, Percentage};

use crate::{
    events::SLSKEvents,
    gui::widgets::chatrooms::ChatroomState,
    styles::{
        STYLE_DEFAULT_HIGHLIGHT_LOW_CONTRAST, STYLE_DEFAULT_LOW_CONTRAST, STYLE_DISABLED_DEFAULT,
        STYLE_FAIL_DEFAULT,
    },
    utils::{key_events_into_paragraph_tabs, vec_string_to_tabs},
};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tokio::sync::RwLock;
use widgets::list::List;

use self::windows::filesearch::FileSearchWindow;
use self::{
    widgets::dropdown::DropdownItem,
    windows::{
        chatrooms::ChatroomsWindow, downloads::DownloadsWindow, login::LoginWindow,
        WidgetWithHints, Window,
    },
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Style, Styled},
    symbols,
    widgets::{Block, Borders},
    Frame, Terminal,
};
use std::sync::atomic::AtomicU16;
use std::sync::Arc;
use std::{error::Error, io, time::Duration};
use tokio::sync::broadcast::{Receiver, Sender};

/// Width, Height
static WINDOW_RESOLUTION: (AtomicU16, AtomicU16) = (AtomicU16::new(0), AtomicU16::new(0));

make_window_enum!(
    ('a),
    LoginWindow get_mut_login 0 ('a),
    ChatroomsWindow get_mut_chatrooms 1 ('a),
    // FileSearchWindow get_mut_filesearch 2 ('a, 'b),
    FileSearchWindow get_mut_filesearch 2 ('a),
    DownloadsWindow get_mut_downloads 3 ('a),
);

#[derive(Clone)]
struct App<'a> {
    windows: Vec<WindowEnum<'a>>,
    current_index: u8,
    select_index: u8,
    focused_widget: u8,
    hints: Vec<(Event, String)>,
}

impl<'a> App<'a> {
    fn render_current_window_on_frame(&self, f: &mut Frame<'_>, area: Rect) {
        match self.get_current_window() {
            WindowEnum::LoginWindow(widget) => f.render_widget(widget.to_owned(), area),
            WindowEnum::ChatroomsWindow(widget) => f.render_widget(widget.to_owned(), area),
            WindowEnum::FileSearchWindow(widget) => f.render_widget(widget.to_owned(), area),
            WindowEnum::DownloadsWindow(widget) => f.render_widget(widget.to_owned(), area),
        }
    }

    fn get_current_window(&self) -> &WindowEnum<'a> {
        &self.windows[self.current_index as usize]
    }

    fn get_window_count(&self) -> usize {
        self.windows.len()
    }
}

impl<'a> Default for App<'a> {
    fn default() -> App<'a> {
        App {
            windows: vec![
                WindowEnum::LoginWindow(LoginWindow::default()),
                WindowEnum::ChatroomsWindow(ChatroomsWindow::default()),
                WindowEnum::FileSearchWindow(FileSearchWindow::default()),
                WindowEnum::DownloadsWindow(DownloadsWindow::default()),
            ],
            current_index: 0,
            select_index: 0,
            focused_widget: 0,
            hints: vec![
                (
                    Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL)),
                    String::from("Change window"),
                ),
                (
                    Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::SHIFT)),
                    String::from("Quit"),
                ),
                (
                    Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::CONTROL)),
                    String::from("Next window"),
                ),
                (
                    Event::Key(KeyEvent::new(
                        KeyCode::Tab,
                        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
                    )),
                    String::from("Previous window"),
                ),
                (
                    Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
                    String::from("Next widget"),
                ),
                (
                    Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT)),
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

    loop {
        let app = App::default();

        if !run_app(&mut terminal, app, gui_queue.resubscribe(), write_queue.clone())? {
            break;
        };
    }

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
) -> io::Result<bool> {
    loop {
        let gui_event = gui_queue.try_recv().ok();
        match gui_event {
            Some(gui_event) => match gui_event {
                SLSKEvents::LoginResult { success, reason } => {
                    let login_window = app.get_mut_login();
                    let label: String;
                    let style: Style;
                    match success {
                        true => {
                            label = String::from("LOGGED IN");
                            style = STYLE_DISABLED_DEFAULT;
                            login_window.login_button.disable();

                            login_window.logout_button.enable();
                            login_window.logout_button.set_label(String::from("LOGOUT"));
                            login_window.logout_button = login_window.logout_button.clone().set_style(STYLE_DEFAULT);
                        }
                        false => {
                            // Unwrapping is safe here because a failed login will always have a reason
                            label = String::from(format!("LOG IN FAILED: {}", reason.unwrap()));
                            style = STYLE_FAIL_DEFAULT;
                        }
                    };
                    login_window.login_button.set_label(label);
                    login_window.login_button = login_window.login_button.clone().set_style(style);
                }
                SLSKEvents::Quit { restart } => return Ok(restart),
                SLSKEvents::TryLogin { .. } => (),
                SLSKEvents::RoomList {
                    mut rooms_and_num_of_users,
                } => {
                    let chatroom_window = app.get_mut_chatrooms();

                    rooms_and_num_of_users.sort_by_key(|k| k.1);
                    rooms_and_num_of_users.reverse();

                    chatroom_window.rooms_dropdown.children = rooms_and_num_of_users
                        .into_iter()
                        .map(|(s, _num)| {
                            if !chatroom_window.chatrooms.contains_key(&s) {
                                chatroom_window
                                    .chatrooms
                                    .insert(s.clone(), (ChatroomState::default(), List::default()));
                            }
                            DropdownItem::empty(s)
                        })
                        .collect();
                }
                SLSKEvents::JoinRoom { .. } => (),
                SLSKEvents::LeaveRoom { .. } => (),
                SLSKEvents::UpdateRoom { room, stats } => {
                    let chatroom_window = app.get_mut_chatrooms();

                    for (user, user_stats) in stats {
                        chatroom_window
                            .chatrooms
                            .get_mut(&room)
                            .unwrap()
                            .0
                            .add_user(user, user_stats);
                    }

                    chatroom_window.update_sidebar();
                }
                SLSKEvents::ChatroomMessage {
                    room,
                    username,
                    message,
                } => match username {
                    Some(username) => {
                        let chatroom_window = app.get_mut_chatrooms();

                        chatroom_window
                            .get_mut_specific_chatroom_state(&room)
                            .unwrap()
                            .add_message(format!("{} [{}] {message}", username, now_as_string()));
                    }
                    None => (),
                },
                SLSKEvents::SearchResults(results) => {
                    let filesearch_window = app.get_mut_filesearch();
                    filesearch_window.add_results(results);
                }
                SLSKEvents::FileSearch { .. } => (),
                SLSKEvents::QueueMessage { .. } => (),
                SLSKEvents::GetInfo(_) => (),
                SLSKEvents::Connect { .. } => (),
                SLSKEvents::NewDownloads {
                    username,
                    folder,
                    files,
                    from_all,
                } => {
                    let downloads_window = app.get_mut_downloads();
                    let files: Vec<_> = files
                        .into_iter()
                        .map(|(filename, filesize)| {
                            (
                                filename,
                                filesize,
                                Arc::new(RwLock::new(DownloadStatus::Queued)),
                                Arc::new(RwLock::new(Percentage(0))),
                            )
                        })
                        .collect();

                    downloads_window.add_folder(username, folder.clone(), files.clone());

                    let _ = &write_queue
                        .send(SLSKEvents::UpdateDownloads {
                            files: files
                                .into_iter()
                                .map(|(filename, _, status, percentage)| {
                                    (format!("{folder}{filename}"), status, percentage)
                                })
                                .collect(),
                            from_all,
                        })
                        .unwrap();
                }
                SLSKEvents::NewDownload {
                    username,
                    folder,
                    filename,
                    filesize,
                } => {
                    let downloads_window = app.get_mut_downloads();

                    let percentage = Arc::new(RwLock::new(Percentage(0)));
                    let status = Arc::new(RwLock::new(DownloadStatus::Queued));

                    downloads_window.add_file(
                        username,
                        folder.clone(),
                        filename.clone(),
                        filesize,
                        status.clone(),
                        percentage.clone(),
                    );

                    let _ = &write_queue
                        .send(SLSKEvents::UpdateDownload {
                            filename: format!("{folder}{filename}"),
                            status,
                            percentage,
                        })
                        .unwrap();
                }
                SLSKEvents::UpdateDownload { .. } => (),
                SLSKEvents::UpdateDownloads { .. } => (),
            },
            None => (),
        }

        terminal.draw(|f| ui(f, &app))?;

        let window_count = app.get_window_count();
        let window = &mut app.windows[app.current_index as usize];
        let window: &mut dyn Window<'_> = match window {
            WindowEnum::LoginWindow(login_window) => login_window,
            WindowEnum::ChatroomsWindow(chatrooms_window) => chatrooms_window,
            WindowEnum::FileSearchWindow(file_search_window) => {
                // if the dialog is visible, we want to treat it as the main window
                if file_search_window.dialog.visible {
                    &mut file_search_window.dialog
                } else {
                    file_search_window
                }
            }
            WindowEnum::DownloadsWindow(downloads_window) => downloads_window,
        };

        if event::poll(Duration::from_millis(25)).unwrap_or(false) == false {
            continue;
        }

        let terminal_event = event::read()?;
        if let Event::Key(key) = terminal_event {
            match key.kind {
                event::KeyEventKind::Press => (),
                event::KeyEventKind::Repeat => (),
                // If Release isn't ignored, double key presses are registered (on Windows)
                event::KeyEventKind::Release => continue,
            };
            app.focused_widget = window.get_focused_index();

            if key.modifiers == (KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Tab => {
                        app.select_index = (app.select_index + 1) % window_count as u8;
                        continue;
                    }
                    KeyCode::Enter => {
                        app.current_index = app.select_index;
                        continue;
                    }
                    _ => (),
                }
            } else if key.modifiers == (KeyModifiers::SHIFT | KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::BackTab => {
                        if app.select_index == 0 {
                            app.select_index = (window_count - 1) as u8
                        } else {
                            app.select_index = (app.select_index - 1) % window_count as u8
                        }
                        continue;
                    }
                    _ => (),
                }
            } else if key.modifiers == (KeyModifiers::SHIFT) {
                match key.code {
                    KeyCode::Esc => {
                        let _ = write_queue.send(SLSKEvents::Quit { restart: false });
                        return Ok(false);
                    }
                    KeyCode::BackTab => {
                        // previous widget
                        app.focused_widget = if app.focused_widget == 0 {
                            window.number_of_widgets() - 1
                        } else {
                            app.focused_widget - 1
                        };
                        window.set_focused_index(app.focused_widget);
                        continue;
                    }
                    _ => (),
                }
            } else {
                match key.code {
                    // next widget
                    KeyCode::Tab => {
                        app.focused_widget = (app.focused_widget + 1) % window.number_of_widgets();
                        window.set_focused_index(app.focused_widget);
                        continue;
                    }
                    _ => (),
                }
            };
            window.perform_action(app.focused_widget, terminal_event, &write_queue);
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let app_windows = &app.windows;
    let current_window = &app_windows[app.current_index as usize];

    let hints = app.hints.clone();
    let current_hints = current_window.get_hints();

    let divider = symbols::line::VERTICAL;

    let (current_hint_paragraph, current_hint_lines) =
        key_events_into_paragraph_tabs(current_hints, Some(divider), f.area().width);
    let current_hint_paragraph = current_hint_paragraph
        .alignment(ratatui::prelude::Alignment::Center)
        .set_style(STYLE_DEFAULT_LOW_CONTRAST);

    let (hint_paragraph, hint_lines) =
        key_events_into_paragraph_tabs(hints, Some(divider), f.area().width);
    let hint_paragraph = hint_paragraph
        .alignment(ratatui::prelude::Alignment::Center)
        .set_style(STYLE_DEFAULT_LOW_CONTRAST)
        .block(Block::default().borders(Borders::TOP));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            // Window selection tabs
            Constraint::Length(1),
            // Window area
            Constraint::Min(0),
            // Widget specific hints
            Constraint::Length(current_hint_lines.max(1)),
            // Generic client hints (+ 1 is to include the top border)
            Constraint::Length(hint_lines + 1),
        ])
        .split(f.area());

    let titles: Vec<String> = app_windows
        .iter()
        .map(|w| w.get_title().to_string())
        .collect();
    let titles = vec_string_to_tabs(
        titles,
        STYLE_DEFAULT_LOW_CONTRAST,
        STYLE_DEFAULT_HIGHLIGHT_LOW_CONTRAST,
        app.select_index as usize,
        app.current_index as usize,
    );

    let terminal_size = f.area();
    WINDOW_RESOLUTION
        .0
        .store(terminal_size.width, std::sync::atomic::Ordering::Release);
    WINDOW_RESOLUTION.1.store(
        terminal_size.height - hint_lines - current_hint_lines,
        std::sync::atomic::Ordering::Release,
    );

    f.render_widget(titles, chunks[0]);
    f.render_widget(current_hint_paragraph, chunks[2]);
    f.render_widget(hint_paragraph, chunks[3]);
    app.render_current_window_on_frame(f, chunks[1]);
}
