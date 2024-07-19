use simple_llama::Token;

use crate::chat::im_channel::{Message, Role};

pub fn echo_assistant(
    tx: crossbeam::channel::Sender<Message>,
    rx: crossbeam::channel::Receiver<Message>,
) -> std::thread::JoinHandle<anyhow::Result<()>> {
    std::thread::spawn(move || {
        while let Ok(input) = rx.recv() {
            match input {
                Message {
                    role: Role::User,
                    contont: Token::End(message),
                } => {
                    let _ = tx.send(Message {
                        role: Role::Assistant,
                        contont: Token::Start,
                    });
                    let _ = tx.send(Message {
                        role: Role::Assistant,
                        contont: Token::End(message),
                    });
                }
                _ => {}
            }
        }
        Ok(())
    })
}

pub struct TerminalApp {
    pub tx: crossbeam::channel::Sender<Message>,
    pub rx: crossbeam::channel::Receiver<Message>,
}

impl TerminalApp {
    pub fn filter(message: &Message) -> Option<Message> {
        if message.role != Role::User {
            Some(message.clone())
        } else {
            None
        }
    }

    fn listen_user_input(tx: crossbeam::channel::Sender<Message>) {
        let stdin = std::io::stdin();
        loop {
            let mut line = String::new();
            let _ = stdin.read_line(&mut line).unwrap();
            if line.starts_with("exit!") {
                break;
            }
            let _ = tx.send(Message {
                role: Role::User,
                contont: Token::End(line),
            });
        }
    }

    pub fn run_loop(self) -> anyhow::Result<()> {
        let (input_tx, input_rx) = crossbeam::channel::unbounded();

        // setup terminal

        // create app and run it
        std::thread::spawn(move || Self::listen_user_input(input_tx));

        loop {
            let input = crossbeam::select! {
                recv(input_rx) -> input =>{
                    if let Ok(input) = input {
                        input
                    }else{
                        break;
                    }
                }
                recv(self.rx) -> message =>{
                    if let Ok(message) = message {
                        message
                    }else{
                        break;
                    }
                }
            };

            println!("{input:?}");

            if let Role::User = input.role {
                let _ = self.tx.send(input);
            }
        }

        Ok(())
    }
}
