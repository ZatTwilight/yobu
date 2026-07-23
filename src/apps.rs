use crate::protocol::Entry;
use freedesktop_desktop_entry::desktop_entries;
use std::{collections::HashMap, io, process::Command};

pub struct Applications {
    pub entries: Vec<Entry>,
    commands: HashMap<String, Vec<String>>,
}

impl Applications {
    pub fn discover() -> Self {
        let locales = locales();
        let mut entries = Vec::new();
        let mut commands = HashMap::new();

        for desktop in desktop_entries(&locales) {
            if desktop.no_display() || desktop.hidden() || desktop.type_() != Some("Application") {
                continue;
            }
            let Some(name) = desktop.name(&locales) else {
                continue;
            };
            let Ok(command) = desktop.parse_exec() else {
                continue;
            };
            if command.is_empty() {
                continue;
            }

            let id = format!("app:{}", desktop.appid);
            commands.insert(id.clone(), command);
            entries.push(Entry {
                id: id.clone(),
                label: name.into_owned(),
                description: desktop.comment(&locales).map(|value| value.into_owned()),
                icon: desktop.icon().map(str::to_owned),
                value: id,
            });
        }

        entries.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
        entries.dedup_by(|a, b| a.id == b.id);
        Self { entries, commands }
    }

    pub fn launch(&self, id: &str) -> io::Result<()> {
        let command = self
            .commands
            .get(id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "application not found"))?;
        Command::new(&command[0]).args(&command[1..]).spawn()?;
        Ok(())
    }
}

fn locales() -> Vec<String> {
    std::env::var("LANGUAGE")
        .ok()
        .map(|value| value.split(':').map(str::to_owned).collect())
        .unwrap_or_else(|| {
            std::env::var("LANG")
                .ok()
                .map(|value| vec![value])
                .unwrap_or_default()
        })
}
