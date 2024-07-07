use crossterm::event;
use lua_llama::Token;
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
