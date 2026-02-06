use std::{mem, rc::Rc};

pub(crate) use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::Alignment,
    prelude::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::Text,
    widgets::{Block, Borders, Clear, Paragraph, Widget, WidgetRef, Wrap},
};
use tokio::sync::broadcast::Sender;

use crate::{
    events::SLSKEvents,
    gui::{
        widgets::button::Button,
        windows::{FocusableWidget, SLSKWidget, WidgetWithHints, Window},
    },
    styles::STYLE_DEFAULT,
};

#[derive(Clone)]
pub(crate) enum DialogType<'a, Args>
where
    Args: Clone,
{
    YesNo {
        yes: Button<'a, (Sender<SLSKEvents>, Args), ()>,
        no: Button<'a, (Sender<SLSKEvents>, Args), ()>,
        question: Paragraph<'a>,
    },
    #[allow(dead_code)]
    Options(Vec<String>),
}

impl<'a, Args> DialogType<'a, Args>
where
    Args: Clone,
{
    pub(crate) fn yes_no(
        yes: String,
        no: String,
        question: String,
        block: Block<'a>,
        style: Style,
        yes_func: Rc<
            dyn for<'b> Fn(
                    &'b Button<'_, (Sender<SLSKEvents>, Args), ()>,
                    (Sender<SLSKEvents>, Args),
                ) + 'static,
        >,
        no_func: Rc<
            dyn for<'b> Fn(
                    &'b Button<'_, (Sender<SLSKEvents>, Args), ()>,
                    (Sender<SLSKEvents>, Args),
                ) + 'static,
        >,
    ) -> Self {
        let yes_button = Button {
            label: yes,
            label_style: STYLE_DEFAULT,
            block: block.clone(),
            event: Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            func: yes_func,
            disabled: false,
        };

        let no_button = Button {
            label: no,
            label_style: STYLE_DEFAULT,
            block: block.clone(),
            event: Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            func: no_func,
            disabled: false,
        };

        let question = Paragraph::new(Text::styled(question, style))
            .alignment(Alignment::Center)
            .block(block.clone());

        Self::YesNo {
            yes: yes_button,
            no: no_button,
            question,
        }
    }
}

impl<Args> Default for DialogType<'_, Args>
where
    Args: Clone,
{
    fn default() -> Self {
        Self::yes_no(
            String::from("Yes"),
            String::from("No"),
            String::from("Yes or No?"),
            Block::new().borders(Borders::ALL).style(STYLE_DEFAULT),
            STYLE_DEFAULT,
            Rc::new(|_, _| {}),
            Rc::new(|_, _| {}),
        )
    }
}

#[derive(Clone)]
pub(crate) struct Dialog<'a, Args>
where
    Args: Clone,
{
    dialog_type: DialogType<'a, Args>,
    pub(crate) block: Block<'a>,
    pub(crate) style: Style,
    in_focus: bool,
    pub(crate) visible: bool,
    focus_index: u8,
    pub(crate) state: Option<Args>,
}

impl<Args> Default for Dialog<'_, Args>
where
    Args: Clone,
{
    fn default() -> Self {
        Self {
            dialog_type: Default::default(),
            block: Block::default()
                .borders(Borders::ALL)
                .border_style(STYLE_DEFAULT),
            style: STYLE_DEFAULT,
            in_focus: false,
            visible: false,
            focus_index: 0,
            state: None,
        }
    }
}

impl<Args> Dialog<'_, Args>
where
    Args: Clone,
{
    pub(crate) fn show(&mut self) {
        self.visible = true;
    }

    pub(crate) fn hide(&mut self) {
        self.visible = false;
    }

    /// No-op if dialog_type is `Options` variant
    pub(crate) fn set_question(&mut self, question: String) {
        match &mut self.dialog_type {
            DialogType::YesNo {
                question: question_paragraph,
                ..
            } => {
                *question_paragraph =
                    Paragraph::new(Text::styled(question, self.style)).alignment(Alignment::Center);
            }
            DialogType::Options(_) => (),
        }
    }
    pub(crate) fn yes_no_funcs(
        &mut self,
        yes_func: Option<
            Rc<
                dyn for<'b> Fn(
                        &'b Button<'_, (Sender<SLSKEvents>, Args), ()>,
                        (Sender<SLSKEvents>, Args),
                    ) + 'static,
            >,
        >,
        no_func: Option<
            Rc<
                dyn for<'b> Fn(
                        &'b Button<'_, (Sender<SLSKEvents>, Args), ()>,
                        (Sender<SLSKEvents>, Args),
                    ) + 'static,
            >,
        >,
    ) -> Self {
        match &mut self.dialog_type {
            DialogType::YesNo { yes, no, .. } => {
                if yes_func.is_some() {
                    yes.func = yes_func.unwrap();
                }
                if no_func.is_some() {
                    no.func = no_func.unwrap();
                }
            }
            DialogType::Options(_) => (),
        };
        self.clone()
    }
}

impl<'a, Args> Dialog<'a, Args>
where
    Args: Clone,
{
    #[allow(dead_code)]
    pub(crate) fn yes_no(
        yes: String,
        no: String,
        question: String,
        block: Block<'a>,
        style: Style,
        yes_func: Rc<
            dyn for<'b> Fn(
                    &'b Button<'_, (Sender<SLSKEvents>, Args), ()>,
                    (Sender<SLSKEvents>, Args),
                ) + 'static,
        >,
        no_func: Rc<
            dyn for<'b> Fn(
                    &'b Button<'_, (Sender<SLSKEvents>, Args), ()>,
                    (Sender<SLSKEvents>, Args),
                ) + 'static,
        >,
    ) -> Self {
        Self {
            dialog_type: DialogType::yes_no(
                yes,
                no,
                question,
                block.clone(),
                style,
                yes_func,
                no_func,
            ),
            block,
            style,
            in_focus: false,
            visible: false,
            focus_index: 0,
            state: None,
        }
    }
}

impl<Args> Widget for Dialog<'_, Args>
where
    Args: Clone,
{
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.visible {
            let center_constraints = [Constraint::Min(0), Constraint::Min(0), Constraint::Min(0)];
            let vertical_center =
                Layout::new(Direction::Vertical, center_constraints).split(area)[1];
            let area =
                Layout::new(Direction::Horizontal, center_constraints).split(vertical_center)[1];
            Clear::render(Clear, area, buf);
            self.block.render_ref(area, buf);
            let area = self.block.inner(area);

            match self.dialog_type {
                DialogType::YesNo {
                    mut question,
                    mut yes,
                    mut no,
                } => {
                    question = question.wrap(Wrap { trim: true });
                    let chunks = Layout::new(
                        Direction::Vertical,
                        [Constraint::Min(0), Constraint::Length(3)],
                    )
                    .split(area);
                    let answer_area = Layout::new(
                        Direction::Horizontal,
                        [Constraint::Fill(1), Constraint::Fill(1)],
                    )
                    .split(chunks[1]);

                    if self.focus_index == 0 {
                        yes.make_focused();
                    } else if self.focus_index == 1 {
                        no.make_focused();
                    } else {
                        unimplemented!()
                    }
                    question.render(chunks[0], buf);
                    yes.render(answer_area[0], buf);
                    no.render(answer_area[1], buf);
                }
                DialogType::Options(_) => todo!(),
            };
        }
    }
}

impl<Args> WidgetWithHints for Dialog<'_, Args>
where
    Args: Clone,
{
    fn get_hints(&self) -> Vec<(Event, String)> {
        let mut hints = match self.get_widget(self.focus_index) {
            Some(widget) => widget.get_hints(),
            _ => Vec::new(),
        };
        if self.visible {
            hints.push((
                Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
                String::from("Close Dialog"),
            ));
        }
        hints
    }
}

impl<Args> FocusableWidget for Dialog<'_, Args>
where
    Args: Clone,
{
    fn make_focused(&mut self) {
        self.in_focus = true;
    }
}

impl<Args> SLSKWidget for Dialog<'_, Args> where Args: Clone {}

impl<Args> Window<'_> for Dialog<'_, Args>
where
    Args: Clone,
{
    fn get_title(&self) -> String {
        String::from("Popup")
    }

    fn number_of_widgets(&self) -> u8 {
        match self.dialog_type {
            DialogType::YesNo { .. } => 2,
            DialogType::Options(_) => 1,
        }
    }

    fn get_widget(&self, index: u8) -> Option<&dyn SLSKWidget> {
        match &self.dialog_type {
            DialogType::YesNo { yes, no, .. } => match index {
                0 => Some(yes),
                1 => Some(no),
                _ => unimplemented!(
                "There are only {} widgets, it's impossible to get the widget with index {index}",
                self.number_of_widgets()
            ),
            },
            DialogType::Options(_) => match index {
                0 => todo!(),
                _ => unimplemented!(
                "There are only {} widgets, it's impossible to get the widget with index {index}",
                self.number_of_widgets()
            ),
            },
        }
    }

    fn get_focused_index(&self) -> u8 {
        self.focus_index
    }

    fn set_focused_index(&mut self, index: u8) {
        self.focus_index = index;
    }

    fn perform_action(&mut self, focus_index: u8, event: Event, write_queue: &Sender<SLSKEvents>) {
        if let Event::Key(key) = event {
            if key == KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE) {
                self.hide();
            }
        }

        match &mut self.dialog_type {
            DialogType::YesNo {
                yes,
                no,
                question: _,
            } => match focus_index {
                0 => {
                    if event == yes.event {
                        if self.state.is_some() {
                            (yes.func)(
                                yes,
                                (
                                    write_queue.clone(),
                                    mem::replace(&mut self.state, None).unwrap(),
                                ),
                            );
                        }
                        self.hide();
                    }
                }
                1 => {
                    if event == no.event {
                        if self.state.is_some() {
                            (no.func)(
                                no,
                                (
                                    write_queue.clone(),
                                    mem::replace(&mut self.state, None).unwrap(),
                                ),
                            );
                        }
                        self.hide();
                    }
                }
                _ => unimplemented!("perform_action({focus_index}, {event:?})"),
            },
            DialogType::Options(_) => todo!(),
        }
    }
}
