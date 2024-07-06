use std::{error::Error, io};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Layout},
    style::{Modifier, Style, Stylize},
    terminal::{Frame, Terminal},
    text::{Line, Text},
    widgets::{Block, Borders, List, Paragraph, ScrollbarState, Tabs},
};

use tui_textarea::{Input, Key, TextArea};

enum InputMode {
    Normal,
    Editing,
}

/// App holds the state of the application
struct App {
    /// Input TextArea
    textarea: TextArea<'static>,
    /// Current input mode
    input_mode: InputMode,
    /// History of recorded messages
    messages: Vec<String>,
    /// Scroll position of the message list
    scroll: u16,

    mouse: (u16, u16),

    vertical_scroll_state: ScrollbarState,
}

impl App {
    fn new_textarea() -> TextArea<'static> {
        let mut textarea = TextArea::default();
        textarea.set_block(Block::bordered().title("Input"));
        textarea
    }

    fn new() -> Self {
        let textarea = Self::new_textarea();

        Self {
            input_mode: InputMode::Normal,
            messages: Vec::new(),
            textarea,
            scroll: 0,
            vertical_scroll_state: ScrollbarState::new(0),
            mouse: (0, 0),
        }
    }

    fn submit_message(&mut self) {
        let mut new_textarea = Self::new_textarea();
        std::mem::swap(&mut self.textarea, &mut new_textarea);
        let lines = new_textarea.into_lines();
        self.messages.push(lines.join(" \n"));
        self.vertical_scroll_state = self
            .vertical_scroll_state
            .content_length(self.messages.len());

        self.down();
    }

    fn up(&mut self) {
        self.scroll = self.scroll.max(1) - 1;
        self.vertical_scroll_state.prev();
    }

    fn down(&mut self) {
        self.scroll = self.scroll + 1;
        self.vertical_scroll_state.next();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let app = App::new();
    let res = run_app(&mut terminal, app);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn handler_input(input: Input, app: &mut App) -> bool {
    match app.input_mode {
        InputMode::Normal => match input.key {
            Key::Char('q') | Key::Esc => {
                return false;
            }
            Key::Enter => {
                app.input_mode = InputMode::Editing;
            }
            _ => {}
        },
        InputMode::Editing => match input.key {
            Key::Esc => {
                app.input_mode = InputMode::Normal;
            }
            Key::Char('s') if input.ctrl => {
                app.submit_message();
            }
            _ => {
                app.textarea.input(input);
            }
        },
    }
    true
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        match event::read()? {
            Event::Mouse(event) => match event.kind {
                event::MouseEventKind::ScrollUp => {
                    app.up();
                }
                event::MouseEventKind::ScrollDown => {
                    app.down();
                }
                event::MouseEventKind::Down(_button) => {
                    app.mouse = (event.row, event.column);
                }
                _ => {}
            },
            input => {
                if !handler_input(input.into(), &mut app) {
                    return Ok(());
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Max(10),
        Constraint::Length(1),
    ]);

    let [tabs_area, messages_area, input_area, help_area] = vertical.areas(f.size());

    let tabs = Tabs::new(vec!["Chat", "Setting"])
        .select(0)
        .padding("[", "]")
        .block(Block::bordered());

    f.render_widget(tabs, tabs_area);

    let (msg, style) = match app.input_mode {
        InputMode::Normal => (
            vec![
                "Press ".into(),
                "q | Esc".bold(),
                " to exit, ".into(),
                "Enter".bold(),
                " to start editing.".bold(),
            ],
            Style::default().add_modifier(Modifier::RAPID_BLINK),
        ),
        InputMode::Editing => (
            vec![
                "Press ".into(),
                "Esc".bold(),
                " to stop editing, ".into(),
                "Enter".bold(),
                " to record the message".into(),
            ],
            Style::default(),
        ),
    };
    let text = Text::from(Line::from(msg)).patch_style(style);
    let help_message = Paragraph::new(text);
    f.render_widget(help_message, help_area);

    f.render_widget(app.textarea.widget(), input_area);

    let max_scroll = (app.messages.len() as i32) - (messages_area.height as i32 - 3);
    app.scroll = app.scroll.min(max_scroll.max(0) as u16);

    let text_vec = app
        .messages
        .iter()
        .map(|s| {
            let mut text = Text::raw(s);
            if s.starts_with("AI") {
                text.lines
                    .iter_mut()
                    .for_each(|l| l.style = Style::default().fg(ratatui::style::Color::Yellow));
            }
            text
        })
        .collect::<Vec<_>>();

    let mut messages = Text::default();
    messages.extend(text_vec.into_iter().flatten());

    let messages = Paragraph::new(messages)
        .gray()
        .block(
            Block::default()
                .borders(Borders::TOP | Borders::BOTTOM)
                .title(format!(
                    "Messages {}:{} {:?} {:?}:{:?}",
                    messages_area.height, max_scroll, app.mouse, input_area, messages_area
                )),
        )
        .scroll((app.scroll, 0));

    f.render_widget(messages, messages_area);
}
