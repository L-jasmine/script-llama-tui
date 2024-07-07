use lua_llama::script_llm::Token;
use tui_textarea::Input;

#[derive(Debug, Clone)]
pub enum InputMessage {
    Token(Token),
    Input(Input),
}
