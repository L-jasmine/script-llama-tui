use std::{collections::HashMap, num::NonZeroU32};

use lua_llama::{
    llm::{self, Content, Role},
    HookLlama, IOHook, Token,
};
use mlua::prelude::*;

use crate::{
    event_message::{self, InputMessage},
    ModelType,
};

pub fn new_lua() -> Result<Lua, LuaError> {
    let lua = Lua::new();

    let send_sms = lua.create_function(
        |lua, (number, sms_msg): (String, String)| -> LuaResult<mlua::Table> {
            let r = lua.create_table()?;
            r.set("status", "ok")?;
            r.set("number", number)?;
            r.set("sms_msg", sms_msg)?;
            Ok(r)
        },
    )?;

    let send_msg = lua.create_function(
        |lua, (room_id, message): (u64, String)| -> LuaResult<mlua::Table> {
            let r = lua.create_table()?;
            r.set("status", "ok")?;
            r.set("room_id", room_id)?;
            r.set("message", message)?;
            Ok(r)
        },
    )?;

    let remember = lua.create_function(
        |lua, (time, text): (u64, String)| -> LuaResult<mlua::Table> {
            let r = lua.create_table()?;
            r.set("status", "ok")?;
            Ok(r)
        },
    )?;

    let get_weather = lua.create_function(|lua, _: ()| -> LuaResult<mlua::Table> {
        let r = lua.create_table()?;
        r.set("status", "ok")?;
        r.set("temp", "18")?;
        r.set("weather", "雨")?;
        Ok(r)
    })?;

    lua.globals().set("send_sms", send_sms)?;
    lua.globals().set("send_msg", send_msg)?;
    lua.globals().set("remember", remember)?;
    lua.globals().set("get_weather", get_weather)?;

    Ok(lua)
}

pub struct LuaHook {
    lua: mlua::Lua,
    code: Option<String>,

    user_rx: crossbeam::channel::Receiver<String>,
    token_tx: crossbeam::channel::Sender<InputMessage>,
}

impl IOHook for LuaHook {
    fn get_input(&mut self) -> anyhow::Result<Option<lua_llama::llm::Content>> {
        if let Some(code) = self.code.take() {
            let s = self
                .lua
                .load(code)
                .eval::<mlua::Value>()
                .map(|v| serde_json::to_string(&v));

            let lua_result = match s {
                Ok(Ok(s)) => {
                    self.token_tx
                        .send(InputMessage::ScriptResult(Ok(s.clone())))?;
                    s
                }
                Ok(Err(err)) => {
                    let s = serde_json::json!(
                        {
                            "status":"error",
                            "error":err.to_string()
                        }
                    )
                    .to_string();
                    self.token_tx
                        .send(InputMessage::ScriptResult(Err(err.to_string())))?;
                    s
                }
                Err(err) => {
                    let s = serde_json::json!(
                        {
                            "status":"error",
                            "error":err.to_string()
                        }
                    )
                    .to_string();
                    self.token_tx
                        .send(InputMessage::ScriptResult(Err(err.to_string())))?;
                    s
                }
            };

            let c = Content {
                role: Role::Tool,
                message: lua_result,
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
            if full_output.is_empty() || full_output.starts_with("--") {
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

impl LuaHook {
    pub fn new(
        user_rx: crossbeam::channel::Receiver<String>,
        token_tx: crossbeam::channel::Sender<InputMessage>,
        lua: Lua,
    ) -> Self {
        Self {
            user_rx,
            token_tx,
            lua,
            code: None,
        }
    }
}

pub fn init_llama(
    cli: crate::Args,
    user_rx: crossbeam::channel::Receiver<String>,
    token_tx: crossbeam::channel::Sender<event_message::InputMessage>,
) -> anyhow::Result<HookLlama<LuaHook>> {
    let prompt = std::fs::read_to_string(&cli.prompt_path)?;
    let mut prompt: HashMap<String, Vec<lua_llama::llm::Content>> = toml::from_str(&prompt)?;
    let sys_prompt = prompt.remove("content").unwrap();

    let model_params: lua_llama::llm::LlamaModelParams =
        lua_llama::llm::LlamaModelParams::default().with_n_gpu_layers(cli.n_gpu_layers);

    let template = match cli.model_type {
        ModelType::Llama3 => llm::llama3::llama3_prompt_template(),
        ModelType::Hermes2ProLlama3 => llm::llama3::hermes_2_pro_llama3_prompt_template(),
        ModelType::Gemma2 => llm::gemma::gemma2_prompt_template(),
        ModelType::Qwen => llm::qwen::qwen_prompt_template(),
    };

    let lua = new_lua()?;
    let hook = LuaHook::new(user_rx, token_tx, lua);

    let llm = llm::LlmModel::new(cli.model_path, model_params, template)?;
    let mut ctx_params =
        llm::LlamaContextParams::default().with_n_ctx(NonZeroU32::new(cli.ctx_size));
    if cli.n_batch > 0 {
        ctx_params = ctx_params.with_n_batch(cli.n_batch);
    }

    let ctx = if !cli.no_full_chat {
        llm::LlamaModelFullPromptContext::new(llm, ctx_params, Some(sys_prompt))
            .unwrap()
            .into()
    } else {
        llm::LlamaModelContext::new(llm, ctx_params, Some(sys_prompt))
            .unwrap()
            .into()
    };

    let lua_llama = HookLlama::new(ctx, hook);

    Ok(lua_llama)
}
