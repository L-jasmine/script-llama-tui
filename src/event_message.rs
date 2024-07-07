use crossterm::event;
use lua_llama::script_llm::{ChatHook, Token};
use tui_textarea::Input;

#[derive(Debug, Clone)]
pub enum InputMessage {
    Token(Token),
    ScriptResult(mlua::Result<String>),
    Input(Input),
}

pub fn listen_input(tx: crossbeam::channel::Sender<InputMessage>) {
    loop {
        match event::read() {
            Ok(input) => {
                let input = input.into();
                tx.send(InputMessage::Input(input))
                    .expect("Failed to send input message");
            }
            Err(_err) => {
                continue;
            }
        }
    }
}

pub struct Hook {
    user_rx: crossbeam::channel::Receiver<String>,
    token_tx: crossbeam::channel::Sender<InputMessage>,
}

impl Hook {
    pub fn new(
        user_rx: crossbeam::channel::Receiver<String>,
        token_tx: crossbeam::channel::Sender<InputMessage>,
    ) -> Self {
        Self { user_rx, token_tx }
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

impl Into<ChatHook<Hook>> for Hook {
    fn into(self) -> ChatHook<Hook> {
        ChatHook {
            data: self,
            get_user_input: Hook::get_user_input,
            token_callback: Hook::token_callback,
            parse_script_result: Hook::parse_script_result,
            parse_script_error: Hook::parse_script_error,
        }
    }
}
