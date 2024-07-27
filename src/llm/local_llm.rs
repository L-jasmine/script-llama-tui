use std::sync::Arc;

use simple_llama::{
    llm::{ChatRequest, LlamaCtx, SimpleOption},
    Content,
};

use crate::chat::im_channel::{Message, MessageRx, MessageTx, Role};

struct ScriptHook {
    rx: MessageRx,
    tx: MessageTx,
}

#[derive(Debug, Clone)]
pub enum Token {
    Start,
    Chunk(String),
    End(String),
}

impl ScriptHook {
    fn get_input(&mut self) -> anyhow::Result<Option<Content>> {
        while let Ok(input) = self.rx.recv() {
            match input {
                Message {
                    role,
                    contont: Token::End(message),
                } if role == Role::User || role == Role::Tool => {
                    let c = simple_llama::llm::Content { role, message };
                    return Ok(Some(c));
                }

                _ => {}
            }
        }
        Ok(None)
    }

    fn token_callback(&mut self, token: Token) -> anyhow::Result<()> {
        self.tx.send(Message {
            role: Role::Assistant,
            contont: token,
        })?;
        Ok(())
    }
}

pub struct LocalLlama {
    ctx: LlamaCtx,
    hook: ScriptHook,
    prompts: Vec<Arc<Content>>,
}

impl LocalLlama {
    pub fn new(ctx: LlamaCtx, prompts: Vec<Arc<Content>>, rx: MessageRx, tx: MessageTx) -> Self {
        let hook = ScriptHook { rx, tx };
        LocalLlama { ctx, hook, prompts }
    }

    pub fn run_loop(&mut self) -> anyhow::Result<()> {
        loop {
            let c = match self.hook.get_input()? {
                Some(c) => c,
                None => return Err(anyhow::anyhow!("input is clone")),
            };
            self.prompts.push(Arc::new(c));

            self.hook.token_callback(Token::Start)?;
            let mut stream = self.ctx.chat(ChatRequest {
                prompts: self.prompts.clone(),
                simple_option: SimpleOption::Temp(0.7),
            })?;

            for token in &mut stream {
                self.hook.token_callback(Token::Chunk(token))?;
            }

            let message: String = stream.into();
            self.hook.token_callback(Token::End(message.clone()))?;
            self.prompts.push(Arc::new(Content {
                role: Role::Assistant,
                message,
            }));
        }
    }

    pub fn filter(message: &Message) -> Option<Message> {
        if message.role != Role::Assistant {
            Some(message.clone())
        } else {
            None
        }
    }
}
