pub type Role = lua_llama::llm::Role;

pub type Chunk = lua_llama::Token;

pub struct MessageConsumer {
    pub filter: fn(message: &Message) -> Option<Message>,
    pub tx: MessageTx,
}

pub trait ChannelMember {
    fn consumer(&self) -> MessageConsumer;
}

#[derive(Clone, Debug)]
pub struct Message {
    pub role: Role,
    pub contont: Chunk,
}

pub type MessageRx = crossbeam::channel::Receiver<Message>;
pub type MessageTx = crossbeam::channel::Sender<Message>;

pub struct ImChannel {
    rx: MessageRx,
    tx: MessageTx,
    consumers: Vec<MessageConsumer>,
    close_rx: crossbeam::channel::Receiver<()>,
}

impl ImChannel {
    pub fn new(close_rx: crossbeam::channel::Receiver<()>) -> Self {
        let (tx, rx) = crossbeam::channel::unbounded();
        ImChannel {
            rx,
            tx,
            consumers: Vec::new(),
            close_rx,
        }
    }

    pub fn register_consumer(&mut self, member: MessageConsumer) {
        self.consumers.push(member);
    }

    pub fn register(&mut self, filter: fn(message: &Message) -> Option<Message>) -> MessageRx {
        let (tx, rx) = crossbeam::channel::unbounded();
        self.consumers.push(MessageConsumer { filter, tx });
        rx
    }

    pub fn new_message_tx(&self) -> MessageTx {
        self.tx.clone()
    }

    pub fn run_loop(&mut self) {
        loop {
            let r = crossbeam::select! {
                    recv(self.close_rx) -> _ => return,
                    recv(self.rx) -> msg => msg
            };

            if let Ok(msg) = r {
                for c in &self.consumers {
                    if let Some(m) = (c.filter)(&msg) {
                        let _ = c.tx.send(m);
                    }
                }
            } else {
                return;
            }
        }
    }
}
