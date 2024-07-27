use std::{collections::HashMap, error::Error, num::NonZeroU32, sync::Arc};

use chat::im_channel;
use clap::Parser;
use llm::local_llm;
use simple_llama::llm::{self as llama, PromptTemplate};
use tool_env::ScriptExecutor;

mod chat;
mod component;
mod debug_tool;
mod llm;
mod tool_env;

#[derive(Debug, clap::Parser)]
struct Args {
    #[arg(long, short, required = true)]
    project_path: String,

    /// full prompt chat
    #[arg(long)]
    debug_ui: bool,

    #[arg(long)]
    debug_llm: bool,

    #[arg(short, long, value_enum, default_value = "none")]
    engine: Engine,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct Project {
    model_path: String,
    prompts: String,
    template: String,
    run: RunOptions,
    templates: HashMap<String, PromptTemplate>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RunOptions {
    #[serde(default)]
    ctx_size: u32,
    #[serde(default)]
    n_batch: u32,
    #[serde(default)]
    n_gpu_layers: u32,
}

impl RunOptions {
    fn fill_default_value(&mut self) {
        if self.ctx_size == 0 {
            self.ctx_size = 1024;
        }
        if self.n_batch == 0 {
            self.n_batch = 512;
        }
        if self.n_gpu_layers == 0 {
            self.n_gpu_layers = 100;
        }
    }
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum Engine {
    None,
    Lua,
    Rhai,
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let cli = Args::parse();
    let mut project: Project =
        toml::from_str(&std::fs::read_to_string(&cli.project_path).unwrap()).unwrap();
    project.run.fill_default_value();

    let (chan_close_tx, chan_close_rx) = crossbeam::channel::bounded(1);

    let mut chan = im_channel::ImChannel::new(chan_close_rx);

    let (tx, rx) = chan.register(tool_env::filter);
    match cli.engine {
        Engine::Lua => {
            std::thread::spawn(move || {
                let script_executor =
                    ScriptExecutor::new(tool_env::lua::new_lua().unwrap(), rx, tx);
                script_executor.run_loop()
            });
        }
        Engine::Rhai => {
            std::thread::spawn(move || {
                let script_executor = ScriptExecutor::new(tool_env::rhai::new_rhai(), rx, tx);
                script_executor.run_loop()
            });
        }
        Engine::None => {}
    };

    let llama_result;

    let (tx, rx) = chan.register(local_llm::LocalLlama::filter);

    if cli.debug_ui {
        llama_result = debug_tool::echo_assistant(tx, rx);
    } else {
        let prompt = std::fs::read_to_string(&project.prompts)
            .map_err(|_| anyhow::anyhow!("prompt file `{}` not found", project.prompts))?;

        let mut prompt: HashMap<String, Vec<simple_llama::llm::Content>> = toml::from_str(&prompt)?;
        let prompts = prompt.remove("content").unwrap();
        let prompts = prompts.into_iter().map(Arc::new).collect();

        let template = project
            .templates
            .get(&project.template)
            .ok_or(anyhow::anyhow!("template not found"))?
            .clone();

        let (wait_tx, wait_rx) = crossbeam::channel::bounded(1);

        llama_result = std::thread::spawn(move || {
            let model_params: simple_llama::llm::LlamaModelParams =
                simple_llama::llm::LlamaModelParams::default()
                    .with_n_gpu_layers(project.run.n_gpu_layers);

            let llm = llama::LlmModel::new(project.model_path, model_params, template)
                .map_err(|e| anyhow::anyhow!(e))?;

            let ctx_params = llama::LlamaContextParams::default()
                .with_n_ctx(NonZeroU32::new(project.run.ctx_size))
                .with_n_batch(project.run.n_batch);

            let ctx = llama::LlamaCtx::new(llm, ctx_params).unwrap();

            let mut local_llama = llm::local_llm::LocalLlama::new(ctx, prompts, rx, tx);
            wait_tx.send(()).unwrap();

            local_llama.run_loop()
        });

        wait_rx.recv()?;
    }

    let res;
    if cli.debug_llm {
        let (tx, rx) = chan.register(debug_tool::TerminalApp::filter);
        let app = debug_tool::TerminalApp { tx, rx };

        std::thread::spawn(move || chan.run_loop());

        res = app.run_loop();
    } else {
        let (tx, rx) = chan.register(component::App::filter);
        let app = component::App::new(rx, tx);

        std::thread::spawn(move || chan.run_loop());

        res = app.run_loop();
    }

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
