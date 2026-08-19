use std::{collections::HashMap, sync::mpsc::{Receiver, Sender}};

use crate::{
    daemon::client::{Client, ClientId},
    protocol::{Envelope, RequestId},
    providers::api::{ProviderEvent, ProviderTrait},
};

pub enum BrokerMessage {
    Connect(Client),
    Disconnect(ClientId),
    Inbound {
        from: ClientId,
        message: Envelope,
    },
    Publish {
        topic: String,
        payload: serde_json::Value,
    },
    WriterFailed {
        id: ClientId,
        error: String,
    },
}

pub struct Broker {
    clients: HashMap<ClientId, Client>,
    providers: HashMap<String, Box<dyn ProviderTrait>>,
    actions: HashMap<String, (String, String)>,
    rx: Receiver<BrokerMessage>,
}

impl Broker {
    pub fn new() -> (Self, Sender<BrokerMessage>) {
        let (tx, rx) = std::sync::mpsc::channel();
        let broker = Self {
            clients: HashMap::new(),
            providers: HashMap::new(),
            actions: HashMap::new(),
            rx,
        };
        (broker, tx)
    }

    pub fn register_provider(
        &mut self,
        provider: Box<dyn ProviderTrait>,
    ) -> Result<(), anyhow::Error> {
        let name = provider.name().to_owned();
        if self.providers.contains_key(&name) {
            return Err(anyhow::anyhow!("provider already registered: {name}"));
        }

        for action in provider.get_actions() {
            let method = format!("{name}.{action}");
            if self.actions.contains_key(&method) {
                return Err(anyhow::anyhow!("action already registered: {method}"));
            }
            self.actions.insert(method, (name.clone(), action.to_owned()));
        }

        self.providers.insert(name, provider);
        Ok(())
    }

    pub fn init_providers(
        &mut self,
        broker_tx: std::sync::mpsc::Sender<BrokerMessage>,
    ) -> Result<(), anyhow::Error> {
        for provider in self.providers.values_mut() {
            provider.init(broker_tx.clone())?;
        }
        Ok(())
    }

    pub fn listen(&mut self) {
        while let Ok(message) = self.rx.recv() {
            match message {
                BrokerMessage::Connect(client) => {
                    self.clients.insert(client.id, client);
                }
                BrokerMessage::Disconnect(id) => {
                    self.clients.remove(&id);
                }
                BrokerMessage::Inbound { from, message } => {
                    self.handle_inbound(from, message);
                }
                BrokerMessage::Publish { topic, payload } => {
                    self.publish(&topic, payload);
                }
                BrokerMessage::WriterFailed { id, error } => {
                    eprintln!("writer failed for client {}: {}", id.0, error);
                    self.clients.remove(&id);
                }
            }
        }
    }

    fn handle_inbound(&mut self, from: ClientId, message: Envelope) {
        match message {
            Envelope::Subscribe { topic } => {
                if let Some(client) = self.clients.get_mut(&from) {
                    client.subscriptions.insert(topic);
                }
            }
            Envelope::Unsubscribe { topic } => {
                if let Some(client) = self.clients.get_mut(&from) {
                    client.subscriptions.remove(&topic);
                }
            }
            Envelope::Request { id, method, params } => {
                self.handle_request(from, id, &method, params);
            }
            Envelope::Hello { .. }
            | Envelope::Response { .. }
            | Envelope::Error { .. }
            | Envelope::Event { .. } => {
                eprintln!("unsupported inbound message from client {}", from.0);
            }
        }
    }

    fn handle_request(
        &mut self,
        from: ClientId,
        id: RequestId,
        method: &str,
        params: serde_json::Value,
    ) {
        let Some((provider_name, action)) = self.actions.get(method).cloned() else {
            self.respond_error(from, id, "method_not_found", method);
            return;
        };

        let result = {
            let Some(provider) = self.providers.get_mut(&provider_name) else {
                self.respond_error(from, id, "provider_unavailable", &provider_name);
                return;
            };
            provider.run_action(&action, params)
        };

        match result {
            Ok(output) => {
                for ProviderEvent { topic, payload } in output.events {
                    self.publish(&topic, payload);
                }
                self.respond(from, id, output.result);
            }
            Err(error) => {
                self.respond_error(from, id, "action_failed", &error.to_string());
            }
        }
    }

    pub fn respond(&self, to: ClientId, id: RequestId, result: serde_json::Value) {
        self.send_to(to, Envelope::Response { id, result });
    }

    pub fn respond_error(&self, to: ClientId, id: RequestId, code: &str, message: &str) {
        self.send_to(
            to,
            Envelope::Error {
                id,
                code: code.to_owned(),
                message: message.to_owned(),
            },
        );
    }

    pub fn send_to(&self, to: ClientId, message: Envelope) {
        if let Some(client) = self.clients.get(&to)
            && let Err(error) = client.send(message)
        {
            eprintln!("failed to send message to client {}: {}", to.0, error);
        }
    }

    pub fn publish(&self, topic: &str, payload: serde_json::Value) {
        for client in self.clients.values() {
            if client.subscriptions.contains(topic)
                && let Err(error) = client.send(Envelope::Event {
                    topic: topic.to_owned(),
                    payload: payload.clone(),
                })
            {
                eprintln!("failed to publish to client {}: {}", client.id.0, error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, sync::mpsc, thread};

    use super::*;
    use crate::providers::api::{ActionResult, ProviderEvent};

    struct TestProvider;

    impl ProviderTrait for TestProvider {
        fn name(&self) -> &str {
            "test"
        }

        fn init(&mut self, _: std::sync::mpsc::Sender<BrokerMessage>) -> Result<(), anyhow::Error> {
            Ok(())
        }

        fn get_actions(&self) -> Vec<String> {
            vec!["echo".into()]
        }

        fn run_action(
            &mut self,
            action: &str,
            _: serde_json::Value,
        ) -> Result<ActionResult, anyhow::Error> {
            assert_eq!(action, "echo");
            Ok(ActionResult {
                result: serde_json::json!({ "ok": true }),
                events: vec![ProviderEvent {
                    topic: "test.changed".into(),
                    payload: serde_json::json!({ "value": 1 }),
                }],
            })
        }
    }

    #[test]
    fn routes_provider_actions_and_publishes_to_subscribers() {
        let (mut broker, tx) = Broker::new();
        broker.register_provider(Box::new(TestProvider)).unwrap();
        let (outbound_tx, outbound_rx) = mpsc::channel();
        let client_id = ClientId(1);
        let client = Client {
            id: client_id,
            name: "test-client".into(),
            capabilities: HashSet::new(),
            outbound: outbound_tx,
            subscriptions: HashSet::new(),
        };

        let handle = thread::spawn(move || broker.listen());
        tx.send(BrokerMessage::Connect(client)).unwrap();
        tx.send(BrokerMessage::Inbound {
            from: client_id,
            message: Envelope::Subscribe {
                topic: "test.changed".into(),
            },
        })
        .unwrap();
        tx.send(BrokerMessage::Inbound {
            from: client_id,
            message: Envelope::Request {
                id: 7,
                method: "test.echo".into(),
                params: serde_json::Value::Null,
            },
        })
        .unwrap();

        let first = outbound_rx.recv().unwrap();
        let second = outbound_rx.recv().unwrap();
        assert!(matches!(first, Envelope::Event { ref topic, .. } if topic == "test.changed"));
        assert!(matches!(second, Envelope::Response { id: 7, .. }));

        drop(tx);
        handle.join().unwrap();
    }
}
