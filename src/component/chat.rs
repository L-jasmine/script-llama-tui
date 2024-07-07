use std::collections::LinkedList;

use lua_llama::llm::{Content, Role};
use lua_llama::Token;
use ratatui::backend::Backend;
use ratatui::style::{Color, Style};
use ratatui::Terminal;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Text},
    widgets::{Block, Paragraph},
    Frame,
};
use tui_textarea::{Key, TextArea};

use crate::event_message::InputMessage;

pub struct MessagesComponent {
    contents: LinkedList<Content>,
    cursor: (u16, u16),
    lock_on_bottom: bool,
}

impl MessagesComponent {
    pub fn new(contents: LinkedList<Content>) -> Self {
        Self {
            contents,
            cursor: (0, 0),
            lock_on_bottom: true,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect)
    where
        Self: Sized,
    {
        let mut text = Text::default();
        for content in &self.contents {
            let style = match content.role {
                Role::Assistant => Style::new().bg(Color::Cyan),
                Role::User => Style::new().bg(Color::Yellow),
                Role::Tool => Style::new().bg(Color::Gray),
                _ => Style::new(),
            };
            text.extend([Line::styled(
                format!("{}:", content.role.to_string().to_uppercase()),
                style,
            )]);
            text.extend(Text::raw(&content.message).style(style));
            text.extend([Line::default().style(style)]);
        }

        let line_n = text.lines.len();

        let max_line = (area.height - 2 - 1) as usize;
        if line_n > max_line {
            let max_cursor = line_n - max_line;
            if self.cursor.0 >= max_cursor as u16 {
                self.lock_on_bottom = true;
            }

            if self.lock_on_bottom {
                self.cursor.0 = max_cursor as u16;
            }
        } else {
            self.cursor.0 = 0;
        }

        let paragraph = Paragraph::new(text)
            .block(Block::bordered().title(format!("{:?}", self.cursor)))
            .scroll(self.cursor);
        frame.render_widget(paragraph, area);
    }

    pub fn handler_input(&mut self, input: InputMessage) {
        match input {
            InputMessage::Token(Token::Start) => self.contents.push_back(Content {
                role: Role::Assistant,
                message: String::with_capacity(64),
            }),
            InputMessage::Token(Token::Chunk(chunk)) => {
                if let Some(content) = self.contents.back_mut() {
                    content.message.push_str(&chunk);
                }
            }
            InputMessage::Token(Token::End(s)) => {
                if let Some(content) = self.contents.back_mut() {
                    content.message = s;
                }
            }

            InputMessage::ScriptResult(s) => match s {
                Ok(s) => {
                    self.contents.push_back(Content {
                        role: Role::Tool,
                        message: s,
                    });
                }
                Err(err) => {
                    self.contents.push_back(Content {
                        role: Role::Tool,
                        message: format!("Error: {}", err.to_string()),
                    });
                }
            },

            InputMessage::Input(input) => match input.key {
                Key::MouseScrollDown => {
                    if input.ctrl {
                        self.cursor.1 += 1;
                    } else {
                        self.cursor.0 += 1;
                    }
                }
                Key::MouseScrollUp => {
                    if input.ctrl {
                        self.cursor.1 = self.cursor.1.max(1) - 1;
                    } else {
                        self.cursor.0 = self.cursor.0.max(1) - 1;
                        self.lock_on_bottom = false;
                    }
                }
                _ => {}
            },
        }
    }
}

pub struct ChatComponent {
    user_tx: crossbeam::channel::Sender<String>,
    token_rx: crossbeam::channel::Receiver<InputMessage>,
    messages: MessagesComponent,
    input: TextArea<'static>,
    exit_n: u8,
    pub event: String,
}

impl ChatComponent {
    pub fn new(
        contents: LinkedList<Content>,
        user_tx: crossbeam::channel::Sender<String>,
        token_rx: crossbeam::channel::Receiver<InputMessage>,
    ) -> Self {
        Self {
            messages: MessagesComponent::new(contents),
            input: Self::new_textarea(),
            exit_n: 0,
            event: String::new(),
            user_tx,
            token_rx,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect)
    where
        Self: Sized,
    {
        let vertical = Layout::vertical([Constraint::Min(5), Constraint::Max(10)]);
        let [messages_area, input_area] = vertical.areas(area);

        self.messages.render(frame, messages_area);
        frame.render_widget(self.input.widget(), input_area);
    }

    fn new_textarea() -> TextArea<'static> {
        let mut textarea = TextArea::default();
        textarea.set_block(Block::bordered().title("Input"));
        textarea
    }

    fn submit_message(&mut self) {
        let mut new_textarea = Self::new_textarea();
        std::mem::swap(&mut self.input, &mut new_textarea);
        let lines = new_textarea.into_lines();
        let message = lines.join("\n");

        self.user_tx.send(message.clone()).unwrap();

        self.messages.contents.push_back(Content {
            role: Role::User,
            message,
        });
        self.messages.lock_on_bottom = true;
    }

    pub fn handler_input<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> bool {
        let input = self.token_rx.recv().unwrap();
        self.event = format!("{:?}", input);
        match input {
            InputMessage::Input(input) if input.key == Key::F(5) => {
                let _ = terminal.clear();
            }
            InputMessage::Input(input) if (input.key == Key::Char('s') && input.ctrl) => {
                self.submit_message();
            }
            InputMessage::Input(input) if input.key == Key::Esc => {
                self.exit_n += 1;
                return self.exit_n < 2;
            }
            InputMessage::Input(input)
                if (input.key != Key::MouseScrollDown && input.key != Key::MouseScrollUp) =>
            {
                self.input.input(input);
            }
            input => {
                self.messages.handler_input(input);
            }
        }
        self.exit_n = 0;
        true
    }
}
