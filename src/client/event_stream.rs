








use matrix_sdk::{
    self, EventEmitter,
};

use tokio::sync::mpsc;




pub enum StateResult {
    Err,
}
unsafe impl Send for StateResult {}

pub struct EventStream {
    send: mpsc::Sender<StateResult>
}
unsafe impl Send for EventStream {}

impl EventStream {
    pub(crate) fn new() -> (Self, mpsc::Receiver<StateResult>) {
        let (send, recv) = mpsc::channel(1024);

        (Self { send, }, recv) 
    }
}

impl EventEmitter for EventStream {
    
}
