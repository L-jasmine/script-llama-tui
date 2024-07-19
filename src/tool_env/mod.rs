use simple_llama::Token;

use crate::chat::im_channel::{self, Message, MessageRx, MessageTx, Role};

pub mod lua;
pub mod rhai;

pub trait ScriptEngin {
    fn eval(&self, code: &str) -> Result<String, String>;
}

pub struct ScriptExecutor<E: ScriptEngin> {
    engine: E,
    rx: MessageRx,
    tx: MessageTx,
}

impl<E: ScriptEngin> ScriptExecutor<E> {
    pub fn new(engine: E, rx: MessageRx, tx: MessageTx) -> Self {
        ScriptExecutor { engine, rx, tx }
    }

    pub fn eval(&self, code: &str) -> Result<String, String> {
        self.engine.eval(code)
    }

    pub fn run_loop(self) {
        while let Ok(input) = self.rx.recv() {
            if let Message {
                role: Role::Assistant,
                contont: Token::End(code),
            } = input
            {
                match self.eval(&code) {
                    Ok(result) => {
                        let message = Message {
                            role: Role::Tool,
                            contont: Token::End(result),
                        };
                        if let Err(_) = self.tx.send(message) {
                            break;
                        }
                    }
                    Err(err) => {
                        let message = Message {
                            role: Role::Tool,
                            contont: Token::End(
                                serde_json::json!(
                                    {
                                        "status":"error",
                                        "error":err
                                    }
                                )
                                .to_string(),
                            ),
                        };
                        if let Err(_) = self.tx.send(message) {
                            break;
                        }
                    }
                }
            }
        }
    }
}

pub fn filter(message: &im_channel::Message) -> Option<im_channel::Message> {
    match message {
        im_channel::Message {
            role: im_channel::Role::Assistant,
            contont: Token::End(contont),
        } => {
            if !contont.is_empty() && !contont.starts_with("//") {
                Some(message.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}
