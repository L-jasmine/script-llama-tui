use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    widgets::{Block, Paragraph, Tabs},
    Frame, Terminal,
};

use crate::chat::im_channel::{Message, MessageRx, MessageTx, Role};

pub mod chat;

pub struct App {
    pub chat: chat::ChatComponent,
    rx: MessageRx,
}

impl App {
    pub fn new(rx: MessageRx, tx: MessageTx) -> Self {
        Self {
            chat: chat::ChatComponent::new(Default::default(), tx),
            rx,
        }
    }

    pub fn render(&mut self, f: &mut Frame) {
        let vertical = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(1),
        ]);

        let [tabs_area, main_area, help_area] = vertical.areas(f.size());

        let tabs = Tabs::new(vec!["Chat", "Setting"])
            .select(0)
            .padding("[", "]")
            .block(Block::bordered());

        f.render_widget(tabs, tabs_area);
        self.chat.render(f, main_area);

        let help_message = Paragraph::new(format!("help... event:{}", self.chat.event));
        f.render_widget(help_message, help_area);
    }

    pub fn run_loop(mut self) -> anyhow::Result<()> {
        let (input_tx, input_rx) = crossbeam::channel::unbounded();

        // setup terminal
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // create app and run it
        std::thread::spawn(move || Self::listen_user_input(input_tx));

        loop {
            terminal.draw(|f| self.render(f))?;

            let input = crossbeam::select! {
                recv(input_rx) -> input =>{
                    if let Ok(input) = input {
                        chat::Input::Event(input)
                    }else{
                        break;
                    }
                }
                recv(self.rx) -> message =>{
                    if let Ok(message) = message {
                        chat::Input::Message(message)
                    }else{
                        break;
                    }
                }
            };

            if !self.chat.handler_input(&mut terminal, input) {
                break;
            }
        }

        // restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;
        Ok(())
    }

    fn listen_user_input(tx: crossbeam::channel::Sender<event::Event>) {
        loop {
            tx.send(event::read().expect("Failed to read event"))
                .expect("Failed to send input message");
        }
    }

    pub fn filter(message: &Message) -> Option<Message> {
        if message.role != Role::User {
            Some(message.clone())
        } else {
            None
        }
    }
}
