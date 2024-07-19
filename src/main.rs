use std::{error::Error, io};

use clap::Parser;
use component::chat::ChatComponent;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Layout},
    terminal::{Frame, Terminal},
    widgets::{Block, Paragraph, Tabs},
};

mod component;
mod event_message;
mod tool_env;

#[derive(Debug, clap::Parser)]
struct Args {
    /// Path to the model
    #[arg(short, long)]
    model_path: String,

    /// path to the prompt
    #[arg(short, long)]
    prompt_path: String,

    /// Type of the model
    #[arg(short('t'), long, value_enum)]
    model_type: ModelType,

    /// full prompt chat
    #[arg(long)]
    no_full_chat: bool,

    /// full prompt chat
    #[arg(long)]
    debug_ui: bool,

    #[arg(short, long, value_enum)]
    engine: Engine,

    #[arg(short, long, default_value = "1024")]
    ctx_size: u32,

    /// Number of layers to run on the GPU
    #[arg(short = 'g', long, default_value = "100")]
    n_gpu_layers: u32,

    #[arg(short, long, default_value = "0")]
    n_batch: u32,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum ModelType {
    Llama3,
    Hermes2ProLlama3,
    Gemma2,
    Qwen,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum Engine {
    Lua,
    Rhai,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Args::parse();

    let (user_tx, user_rx) = crossbeam::channel::unbounded();
    let (token_tx, token_rx) = crossbeam::channel::unbounded();

    let token_tx_ = token_tx.clone();

    let app = ChatComponent::new(Default::default(), user_tx, token_rx);

    let llama_result;

    if cli.debug_ui {
        llama_result = std::thread::spawn(move || {
            while let Ok(input) = user_rx.recv() {
                let _ = token_tx.send(event_message::InputMessage::Token(lua_llama::Token::Start));
                std::thread::sleep(std::time::Duration::from_secs(1));
                let _ = token_tx.send(event_message::InputMessage::Token(lua_llama::Token::End(
                    input,
                )));
            }
            Ok(())
        });
    } else {
        let (wait_tx, wait_rx) = crossbeam::channel::bounded(1);

        llama_result = std::thread::spawn(move || match cli.engine {
            Engine::Lua => {
                let mut lua_llama = tool_env::lua::init_llama(cli, user_rx, token_tx)?;
                wait_tx.send(())?;
                lua_llama.chat()
            }
            Engine::Rhai => {
                let mut rhai_llama = tool_env::rhai::init_llama(cli, user_rx, token_tx)?;
                wait_tx.send(())?;
                rhai_llama.chat()
            }
        });

        wait_rx.recv()?;
    }

    std::thread::spawn(move || {
        event_message::listen_input(token_tx_);
    });

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let res = run_app(&mut terminal, app);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    let llama_result = llama_result.join().unwrap();
    if let Err(err) = llama_result {
        println!("llama_result err:{err}")
    }

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: ChatComponent) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;
        if !app.handler_input(terminal) {
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
