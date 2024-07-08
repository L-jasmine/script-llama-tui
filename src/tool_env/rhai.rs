use std::{collections::HashMap, num::NonZeroU32};

use lua_llama::{
    llm::{self, Content, Role},
    HookLlama, IOHook, Token,
};
use rhai::{
    serde::{from_dynamic, to_dynamic},
    Dynamic, Engine, NativeCallContext,
};

use crate::{
    event_message::{self, InputMessage},
    ModelType,
};

fn send_sms(_number: String, _sms_msg: String) -> Result<Dynamic, Box<rhai::EvalAltResult>> {
    let s = serde_json::json!({
        "status":"ok"
    });
    to_dynamic(s)
}

fn send_msg(_room_id: i64, _sms_msg: String) -> Result<Dynamic, Box<rhai::EvalAltResult>> {
    let s = serde_json::json!({
        "status":"ok"
    });
    to_dynamic(s)
}

fn get_weather() -> Result<Dynamic, Box<rhai::EvalAltResult>> {
    let s = serde_json::json!({
        "status":"ok",
        "temp":"18",
        "weather":"雨"
    });
    to_dynamic(s)
}

fn get_current_time(_context: &NativeCallContext) -> Result<Dynamic, Box<rhai::EvalAltResult>> {
    let time = std::time::SystemTime::now();
    let s = serde_json::json!({
        "status":"ok",
        "time": time
    });
    to_dynamic(s)
}

fn new_rhai() -> Engine {
    let mut engine = Engine::new();
    engine
        .register_fn("send_sms", send_sms)
        .register_fn("send_msg", send_msg)
        .register_fn("get_weather", get_weather)
        .register_fn("get_current_time", get_current_time);
    engine
}

pub struct RhaiHook {
    rhai: rhai::Engine,
    code: Option<String>,

    user_rx: crossbeam::channel::Receiver<String>,
    token_tx: crossbeam::channel::Sender<InputMessage>,
}

impl IOHook for RhaiHook {
    fn get_input(&mut self) -> anyhow::Result<Option<lua_llama::llm::Content>> {
        if let Some(code) = self.code.take() {
            let s = self
                .rhai
                .eval::<rhai::Dynamic>(&code)
                .and_then(|d| from_dynamic::<serde_json::Value>(&d));
            let result = match s {
                Ok(s) => {
                    self.token_tx
                        .send(InputMessage::ScriptResult(Ok(s.to_string())))?;
                    s
                }
                Err(err) => {
                    let s = serde_json::json!(
                        {
                            "status":"error",
                            "error":err.to_string()
                        }
                    );

                    self.token_tx
                        .send(InputMessage::ScriptResult(Err(err.to_string())))?;
                    s
                }
            };

            let c = Content {
                role: Role::Tool,
                message: result.to_string(),
            };
            return Ok(Some(c));
        };

        let input = self.user_rx.recv().ok();
        if let Some(input) = input {
            let c = Content {
                role: Role::User,
                message: input,
            };
            Ok(Some(c))
        } else {
            Ok(None)
        }
    }

    fn token_callback(&mut self, token: Token) -> anyhow::Result<()> {
        if let Token::End(full_output) = &token {
            if full_output.is_empty() || full_output.starts_with("//") {
            } else {
                self.code = Some(full_output.clone())
            }
        }
        self.token_tx.send(InputMessage::Token(token))?;
        Ok(())
    }

    fn parse_input(&mut self, content: &mut lua_llama::llm::Content) {
        match content.role {
            Role::User => {
                content.message = serde_json::json!({
                    "role":"user",
                    "message":content.message
                })
                .to_string();
            }
            Role::Tool => {
                content.role = Role::User;
                content.message = format!("{{ \"role\":\"tool\",\"message\":{}}}", content.message);
            }
            _ => {}
        }
    }
}

impl RhaiHook {
    pub fn new(
        user_rx: crossbeam::channel::Receiver<String>,
        token_tx: crossbeam::channel::Sender<InputMessage>,
        rhai: Engine,
    ) -> Self {
        Self {
            user_rx,
            token_tx,
            rhai,
            code: None,
        }
    }
}

pub fn init_llama(
    cli: crate::Args,
    user_rx: crossbeam::channel::Receiver<String>,
    token_tx: crossbeam::channel::Sender<event_message::InputMessage>,
) -> anyhow::Result<HookLlama<RhaiHook>> {
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

    let rhai = new_rhai();
    let hook = RhaiHook::new(user_rx, token_tx, rhai);

    let llm = llm::LlmModel::new(cli.model_path, model_params, template)?;
    let ctx = if !cli.no_full_chat {
        let ctx_params = llm::LlamaContextParams::default().with_n_ctx(NonZeroU32::new(1024));
        llm::LlamaModelFullPromptContext::new(llm, ctx_params, Some(sys_prompt))
            .unwrap()
            .into()
    } else {
        let ctx_params = llm::LlamaContextParams::default().with_n_ctx(NonZeroU32::new(1024));
        llm::LlamaModelContext::new(llm, ctx_params, Some(sys_prompt))
            .unwrap()
            .into()
    };

    let lua_llama = HookLlama::new(ctx, hook);

    Ok(lua_llama)
}
