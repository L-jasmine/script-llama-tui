use std::{collections::HashMap, error::Error, num::NonZeroU32};

use chat::im_channel::{self, Message, Role};
use clap::Parser;
use llm::local_llm;
use lua_llama::{
    llm::{self as llama},
    Token,
};
use tool_env::ScriptExecutor;

mod chat;
mod component;
mod llm;
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

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ModelType {
    Llama3,
    Hermes2ProLlama3,
    Gemma2,
    Qwen,
}

impl Into<llama::LlmPromptTemplate> for ModelType {
    fn into(self) -> llama::LlmPromptTemplate {
        match self {
            ModelType::Llama3 => llama::llama3::llama3_prompt_template(),
            ModelType::Hermes2ProLlama3 => llama::llama3::hermes_2_pro_llama3_prompt_template(),
            ModelType::Gemma2 => llama::gemma::gemma2_prompt_template(),
            ModelType::Qwen => llama::qwen::qwen_prompt_template(),
        }
    }
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum Engine {
    Lua,
    Rhai,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Args::parse();

    let (chan_close_tx, chan_close_rx) = crossbeam::channel::bounded(1);

    let mut chan = im_channel::ImChannel::new(chan_close_rx);

    let (tx, rx) = chan.register(component::App::filter);
    let app = component::App::new(rx, tx);

    let (tx, rx) = chan.register(tool_env::filter);
    match cli.engine {
        Engine::Lua => std::thread::spawn(move || {
            let script_executor = ScriptExecutor::new(tool_env::lua::new_lua().unwrap(), rx, tx);
            script_executor.run_loop()
        }),
        Engine::Rhai => std::thread::spawn(move || {
            let script_executor = ScriptExecutor::new(tool_env::rhai::new_rhai(), rx, tx);
            script_executor.run_loop()
        }),
    };

    let llama_result;

    let (tx, rx) = chan.register(local_llm::LocalLlama::filter);

    if cli.debug_ui {
        llama_result = std::thread::spawn(move || {
            while let Ok(input) = rx.recv() {
                match input {
                    Message {
                        role: Role::User,
                        contont: Token::End(message),
                    } => {
                        let _ = tx.send(Message {
                            role: Role::Assistant,
                            contont: Token::Start,
                        });
                        let _ = tx.send(Message {
                            role: Role::Assistant,
                            contont: Token::End(message),
                        });
                    }
                    _ => {}
                }
            }
            Ok(())
        });
    } else {
        let prompt = std::fs::read_to_string(&cli.prompt_path)?;
        let mut prompt: HashMap<String, Vec<lua_llama::llm::Content>> = toml::from_str(&prompt)?;
        let prompts = prompt.remove("content").unwrap();

        let template = cli.model_type.into();

        let (wait_tx, wait_rx) = crossbeam::channel::bounded(1);

        llama_result = std::thread::spawn(move || {
            let model_params: lua_llama::llm::LlamaModelParams =
                lua_llama::llm::LlamaModelParams::default().with_n_gpu_layers(cli.n_gpu_layers);

            let llm = llama::LlmModel::new(cli.model_path, model_params, template)
                .map_err(|e| anyhow::anyhow!(e))?;

            let mut ctx_params =
                llama::LlamaContextParams::default().with_n_ctx(NonZeroU32::new(cli.ctx_size));
            if cli.n_batch > 0 {
                ctx_params = ctx_params.with_n_batch(cli.n_batch);
            }

            let ctx = if cli.no_full_chat {
                llama::LlamaModelContext::new(llm, ctx_params, Some(prompts))
                    .unwrap()
                    .into()
            } else {
                llama::LlamaModelFullPromptContext::new(llm, ctx_params, Some(prompts))
                    .unwrap()
                    .into()
            };

            let mut local_llama = llm::local_llm::LocalLlama::new(ctx, rx, tx);
            wait_tx.send(()).unwrap();

            local_llama.run_loop()
        });

        wait_rx.recv()?;
    }

    std::thread::spawn(move || chan.run_loop());

    let res = app.run_loop();

    let _ = chan_close_tx.send(());

    let llama_result = llama_result.join().unwrap();
    if let Err(err) = llama_result {
        println!("llama_result err:{err}")
    }

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}
