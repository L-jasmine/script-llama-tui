use std::{error::Error, io};

use component::chat::ChatComponent;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Layout},
    terminal::{Frame, Terminal},
    widgets::{Block, Paragraph, Tabs},
};

use tui_textarea::Input;

mod component;
mod event_message;

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let app = ChatComponent::new(Default::default());
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

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: ChatComponent) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;
        let input: Input = event::read()?.into();
        if !app.handler_input(event_message::InputMessage::Input(input)) {
            return Ok(());
        }
    }
}

fn ui(f: &mut Frame, app: &mut ChatComponent) {
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
    app.render(f, main_area);

    let help_message = Paragraph::new(format!("help... event:{}", app.event));
    f.render_widget(help_message, help_area);
}
