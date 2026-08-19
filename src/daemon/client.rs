use std::{collections::HashSet, sync::mpsc::Sender};

use crate::protocol::Envelope;

#[derive(Eq, Hash, PartialEq, Copy, Clone)]
pub struct ClientId(pub u64);

pub struct Client {
    pub id: ClientId,
    pub name: String,
    pub capabilities: HashSet<String>,
    pub outbound: Sender<Envelope>,
    pub subscriptions: HashSet<String>,
}

impl Client {
    pub fn send(&self, message: Envelope) -> Result<(), std::sync::mpsc::SendError<Envelope>> {
        self.outbound.send(message)
    }
}
