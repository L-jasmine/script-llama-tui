use std::{collections::HashMap, error::Error, io, num::NonZeroU32, sync::Arc};

use clap::Parser;
use component::chat::ChatComponent;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use event_message::Hook;
use lua_llama::{llm, script_llm::LuaLlama};
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
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum ModelType {
    Llama3,
    Hermes2ProLlama3,
    Gemma2,
    Qwen,
}

fn init_llama(
    cli: Args,
    user_rx: crossbeam::channel::Receiver<String>,
    token_tx: crossbeam::channel::Sender<event_message::InputMessage>,
) -> anyhow::Result<LuaLlama<Hook>> {
    let prompt = std::fs::read_to_string(&cli.prompt_path)?;
    let mut prompt: HashMap<String, Vec<lua_llama::llm::Content>> = toml::from_str(&prompt)?;
    let sys_prompt = prompt.remove("content").unwrap();

    let model_params: lua_llama::llm::LlamaModelParams =
        lua_llama::llm::LlamaModelParams::default().with_n_gpu_layers(512);

    let template = match cli.model_type {
        ModelType::Llama3 => llm::llama3::llama3_prompt_template(),
        ModelType::Hermes2ProLlama3 => llm::llama3::hermes_2_pro_llama3_prompt_template(),
        ModelType::Gemma2 => llm::gemma::gemma2_prompt_template(),
        ModelType::Qwen => llm::qwen::qwen_prompt_template(),
    };

    let lua = tool_env::new_lua()?;

    let llm = llm::LlmModel::new(cli.model_path, model_params, template)?;
    let ctx = if !cli.no_full_chat {
        let ctx_params = llm::LlamaContextParams::default().with_n_ctx(NonZeroU32::new(1024 * 2));
        llm::LlamaModelFullPromptContext::new(llm, ctx_params, Some(sys_prompt))
            .unwrap()
            .into()
    } else {
        let ctx_params = llm::LlamaContextParams::default().with_n_ctx(NonZeroU32::new(1024 * 2));
        llm::LlamaModelContext::new(llm, ctx_params, Some(sys_prompt))
            .unwrap()
            .into()
    };

    let lua_llama = LuaLlama {
        llm: ctx,
        lua,
        hook: event_message::Hook::new(user_rx, token_tx).into(),
    };

    Ok(lua_llama)
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Args::parse();

    let (user_tx, user_rx) = crossbeam::channel::unbounded();
    let (token_tx, token_rx) = crossbeam::channel::unbounded();
    let (wait_tx, wait_rx) = crossbeam::channel::bounded(1);

    let token_tx_ = token_tx.clone();

    let app = ChatComponent::new(Default::default(), user_tx, token_rx);

    let llama_result = std::thread::spawn(move || {
        let mut lua_llama = init_llama(cli, user_rx, token_tx)?;
        wait_tx.send(())?;
        lua_llama.chat()
    });

    wait_rx.recv()?;

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
        if !app.handler_input() {
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
