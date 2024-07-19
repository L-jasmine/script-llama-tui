use lua_llama::{HookLlama, IOHook, Token};

use crate::chat::im_channel::{self, Message, MessageRx, MessageTx, Role};

struct ScriptHook {
    rx: MessageRx,
    tx: MessageTx,
}

impl IOHook for ScriptHook {
    fn get_input(&mut self) -> anyhow::Result<Option<lua_llama::llm::Content>> {
        while let Ok(input) = self.rx.recv() {
            match input {
                Message {
                    role,
                    contont: Token::End(message),
                } if role == Role::User || role == Role::Tool => {
                    let c = lua_llama::llm::Content { role, message };
                    return Ok(Some(c));
                }

                _ => {}
            }
        }
        Ok(None)
    }

    fn token_callback(&mut self, token: lua_llama::Token) -> anyhow::Result<()> {
        self.tx.send(Message {
            role: Role::Assistant,
            contont: token,
        })?;
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

pub struct LocalLlama {
    hook: HookLlama<ScriptHook>,
}

impl LocalLlama {
    pub fn new(ctx: lua_llama::LlamaCtx, rx: MessageRx, tx: MessageTx) -> Self {
        let hook = HookLlama::new(ctx, ScriptHook { rx, tx });

        LocalLlama { hook }
    }

    pub fn run_loop(&mut self) -> anyhow::Result<()> {
        self.hook.chat()
    }

    pub fn filter(message: &Message) -> Option<Message> {
        if message.role != Role::Assistant {
            Some(message.clone())
        } else {
            None
        }
    }
}
