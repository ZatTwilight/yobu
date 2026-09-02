use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use serde_json::json;

use crate::{
    apps::Applications,
    config::{self, SavedState},
    daemon::broker::BrokerMessage,
    providers::api::{ActionResult, ProviderEvent, ProviderTrait},
    search::SearchIndex,
};

struct Catalog {
    applications: Applications,
    search: SearchIndex,
}

impl Catalog {
    fn discover() -> Self {
        let applications = Applications::discover();
        let search = SearchIndex::new(applications.entries.clone());
        Self {
            applications,
            search,
        }
    }
}

pub struct ApplicationsProvider {
    catalog: Arc<Mutex<Catalog>>,
    state: Arc<Mutex<SavedState>>,
}

#[derive(Deserialize)]
struct SearchParams {
    query: String,
    #[serde(default = "default_max_results")]
    max_results: usize,
}

#[derive(Deserialize)]
struct LaunchParams {
    id: String,
}

fn default_max_results() -> usize {
    50
}

fn should_queue_event(result: &notify::Result<notify::Event>) -> bool {
    // Catalog discovery reads watched files. Access events must not trigger
    // another scan or reset the debounce timer. Preserve errors/rescan events.
    !matches!(result, Ok(event) if matches!(event.kind, EventKind::Access(_)))
}

impl ApplicationsProvider {
    pub fn new() -> Self {
        Self::with_shared(Arc::new(Mutex::new(config::load_state())))
    }

    pub fn with_shared(state: Arc<Mutex<SavedState>>) -> Self {
        Self {
            catalog: Arc::new(Mutex::new(Catalog::discover())),
            state,
        }
    }

    fn refresh(&self) -> usize {
        let refreshed = Catalog::discover();
        let count = refreshed.applications.entries.len();
        *self.catalog.lock().unwrap() = refreshed;
        count
    }

    fn start_watcher(
        catalog: Arc<Mutex<Catalog>>,
        broker_tx: std::sync::mpsc::Sender<BrokerMessage>,
    ) -> Result<(), anyhow::Error> {
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        let mut watcher = RecommendedWatcher::new(
            move |result| {
                if !should_queue_event(&result) {
                    return;
                }
                if event_tx.send(result).is_err() {
                    eprintln!("application watcher event receiver stopped");
                }
            },
            notify::Config::default(),
        )?;

        let mut watched = 0;
        for path in freedesktop_desktop_entry::default_paths() {
            if path.is_dir() {
                watcher.watch(&path, RecursiveMode::NonRecursive)?;
                watched += 1;
            }
        }
        if watched == 0 {
            return Err(anyhow::anyhow!(
                "no application directories available to watch"
            ));
        }

        thread::spawn(move || {
            // Keeping the watcher in this thread keeps its OS watches alive.
            let _watcher = watcher;
            while let Ok(event) = event_rx.recv() {
                if let Err(error) = event {
                    eprintln!("application watcher error: {error}");
                    continue;
                }

                // Package managers commonly emit a burst of create/write/rename
                // events. Wait until the directory has been quiet for 500 ms.
                loop {
                    match event_rx.recv_timeout(Duration::from_millis(500)) {
                        Ok(Ok(_)) => continue,
                        Ok(Err(error)) => {
                            eprintln!("application watcher error: {error}");
                            continue;
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                    }
                }

                let refreshed = Catalog::discover();
                let count = refreshed.applications.entries.len();
                *catalog.lock().unwrap() = refreshed;
                if broker_tx
                    .send(BrokerMessage::Publish {
                        topic: "applications.changed".into(),
                        payload: json!({ "count": count }),
                    })
                    .is_err()
                {
                    return;
                }
            }
        });
        Ok(())
    }
}

impl Default for ApplicationsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderTrait for ApplicationsProvider {
    fn name(&self) -> &str {
        "applications"
    }

    fn init(
        &mut self,
        broker_tx: std::sync::mpsc::Sender<BrokerMessage>,
    ) -> Result<(), anyhow::Error> {
        Self::start_watcher(self.catalog.clone(), broker_tx)
    }

    fn get_actions(&self) -> Vec<String> {
        vec![
            "list".into(),
            "search".into(),
            "launch".into(),
            "refresh".into(),
        ]
    }

    fn run_action(
        &mut self,
        action: &str,
        params: serde_json::Value,
    ) -> Result<ActionResult, anyhow::Error> {
        match action {
            "list" => {
                let catalog = self.catalog.lock().unwrap();
                Ok(ActionResult {
                    result: json!({ "entries": &catalog.applications.entries }),
                    events: Vec::new(),
                })
            }
            "search" => {
                let params: SearchParams = serde_json::from_value(params)?;
                let state = self.state.lock().unwrap().clone();
                let entries = self.catalog.lock().unwrap().search.search(
                    &params.query,
                    params.max_results,
                    &state,
                );
                Ok(ActionResult {
                    result: json!({ "query": params.query, "entries": entries }),
                    events: Vec::new(),
                })
            }
            "launch" => {
                let params: LaunchParams = serde_json::from_value(params)?;
                self.catalog
                    .lock()
                    .unwrap()
                    .applications
                    .launch(&params.id)?;
                {
                    let mut state = self.state.lock().unwrap();
                    state.record_application(&params.id);
                    if let Err(e) = config::save_state(&state) {
                        eprintln!("failed to save frecency: {e}");
                    }
                }
                Ok(ActionResult {
                    result: json!({ "launched": params.id }),
                    events: Vec::new(),
                })
            }
            "refresh" => {
                let count = self.refresh();
                Ok(ActionResult {
                    result: json!({ "count": count }),
                    events: vec![ProviderEvent {
                        topic: "applications.changed".into(),
                        payload: json!({ "count": count }),
                    }],
                })
            }
            _ => Err(anyhow::anyhow!("unknown applications action: {action}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, AccessMode, CreateKind, ModifyKind, RemoveKind};

    #[test]
    fn access_events_never_reach_the_debounce_queue() {
        for kind in [
            AccessKind::Any,
            AccessKind::Read,
            AccessKind::Open(AccessMode::Read),
            AccessKind::Close(AccessMode::Read),
            AccessKind::Close(AccessMode::Write),
            AccessKind::Other,
        ] {
            assert!(!should_queue_event(&Ok(notify::Event::new(
                EventKind::Access(kind)
            ))));
        }
    }

    #[test]
    fn changes_rescans_and_errors_still_reach_the_queue() {
        for kind in [
            EventKind::Create(CreateKind::Any),
            EventKind::Modify(ModifyKind::Any),
            EventKind::Remove(RemoveKind::Any),
            EventKind::Any,
            EventKind::Other,
        ] {
            assert!(should_queue_event(&Ok(notify::Event::new(kind))));
        }
        assert!(should_queue_event(&Err(notify::Error::generic(
            "watch error"
        ))));
    }
}
