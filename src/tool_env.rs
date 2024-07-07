use lua_llama::{
    llm::{Content, Role},
    IOHook, Token,
};
use mlua::prelude::*;

use crate::event_message::InputMessage;

pub fn new_lua() -> Result<Lua, LuaError> {
    let lua = Lua::new();

    let send_sms = lua.create_function(
        |_, (number, sms_msg): (String, String)| -> LuaResult<String> {
            let s = serde_json::json!({
                "status":"ok"
            })
            .to_string();

            Ok(s)
        },
    )?;

    let send_msg = lua.create_function(
        |_, (room_id, message): (u64, String)| -> LuaResult<String> {
            let s = serde_json::json!({
                "status":"ok"
            })
            .to_string();

            Ok(s)
        },
    )?;

    let remember = lua.create_function(|_, (time, text): (u64, String)| -> LuaResult<String> {
        let s = serde_json::json!({
            "status":"ok"
        })
        .to_string();
        Ok(s)
    })?;

    let get_weather =
        lua.create_function(|_, _: ()| -> LuaResult<String> { Ok("下雨".to_string()) })?;

    lua.globals().set("send_sms", send_sms)?;
    lua.globals().set("send_msg", send_msg)?;
    lua.globals().set("remember", remember)?;
    lua.globals().set("get_weather", get_weather)?;

    Ok(lua)
}

pub struct LuaHook {
    lua: mlua::Lua,
    lua_code: Option<String>,

    user_rx: crossbeam::channel::Receiver<String>,
    token_tx: crossbeam::channel::Sender<InputMessage>,
}

impl IOHook for LuaHook {
    fn get_input(&mut self) -> anyhow::Result<Option<lua_llama::llm::Content>> {
        if let Some(lua_code) = self.lua_code.take() {
            let s = self.lua.load(lua_code).eval::<Option<String>>();

            let r = match s {
                Ok(Some(s)) => {
                    self.token_tx
                        .send(InputMessage::ScriptResult(Ok(s.clone())))?;
                    Some(s)
                }
                Ok(None) => None,
                Err(err) => {
                    let s = serde_json::json!(
                        {
                            "status":"error",
                            "error":err.to_string()
                        }
                    )
                    .to_string();
                    self.token_tx.send(InputMessage::ScriptResult(Err(err)))?;
                    Some(s)
                }
            };

            if let Some(lua_result) = r {
                let c = Content {
                    role: Role::Tool,
                    message: lua_result,
                };
                return Ok(Some(c));
            }
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
                self.lua_code = Some(full_output.clone())
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
            lua_code: None,
        }
    }

    pub fn get_user_input(&mut self) -> anyhow::Result<Option<String>> {
        Ok(self.user_rx.recv().ok())
    }
    pub fn token_callback(&mut self, token: Token) -> anyhow::Result<()> {
        self.token_tx.send(InputMessage::Token(token))?;
        Ok(())
    }
    pub fn parse_script_result(&mut self, result: &str) -> anyhow::Result<String> {
        self.token_tx
            .send(InputMessage::ScriptResult(Ok(result.to_string())))?;

        let message = format!("{{ \"role\":\"tool\",\"message\":{result}}}");
        Ok(message)
    }
    pub fn parse_script_error(&mut self, err: mlua::Error) -> anyhow::Result<String> {
        let message = serde_json::json!({
            "role":"tool",
            "message":{
                "status":"error",
                "error":err.to_string()
            }
        })
        .to_string();

        self.token_tx.send(InputMessage::ScriptResult(Err(err)))?;

        Ok(message)
    }
}
