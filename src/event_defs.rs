use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EventFieldDefinition {
    pub name: String,
    pub out_type: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EventDefinition {
    pub message: Option<String>,
    pub fields: Vec<EventFieldDefinition>,
}

// Aliased type for ProviderName -> EventID -> Version -> EventDefinition
pub type EventDefinitions = BTreeMap<String, BTreeMap<u64, BTreeMap<u64, EventDefinition>>>;

pub fn import_metadata_from_system() -> Result<EventDefinitions, String> {
    let mut field_defs = BTreeMap::new();

    info!("Importing metadata from live system, this may take a while...
       (use --no-system-metadata if you don't care about message strings, field names
        and types, or use --export-metadata then --import-metadata to only do it once)");

    for provider_name in crate::windows::get_evt_provider_names()? {
        verbose!("Querying provider {}", provider_name);

        match crate::windows::get_evt_provider_event_fields(&provider_name) {
            Ok(events) => field_defs.insert(provider_name, events),
            Err(e) => {
                warn!("Unable to enumerate events from provider '{}': error {}",
                          provider_name, e);
                continue;
            },
        };
    }
    Ok(field_defs)
}

pub fn update_metadata_with(known_defs: &mut EventDefinitions, new_defs: &EventDefinitions) {
    for (provider_name, new_events) in new_defs {
        let known_events = known_defs.entry(provider_name.to_owned()).or_insert(BTreeMap::new());
        for (eventid, new_versions) in new_events {
            let known_versions = known_events.entry(eventid.to_owned()).or_insert(BTreeMap::new());
            for (version, new_def) in new_versions {
                match known_versions.get_mut(version) {
                    // If we didn't know anything about that event, use it all, it can't be worse
                    None => { known_versions.insert(version.to_owned(), new_def.to_owned()); },
                    // We had an event definition, use contents to fill the blanks
                    Some(event_def) => {
                        if let Some(message) = &new_def.message {
                            event_def.message = Some(message.to_owned());
                        }
                        for (i, def) in new_def.fields.iter().enumerate() {
                            event_def.fields[i] = def.to_owned();
                        }
                    }
                }
            }
        }
    }
}

pub fn export_metadata_to_file(field_defs: &EventDefinitions,
                               out_file: &mut dyn std::io::Write,
                               json_pretty: bool) -> Result<(), String> {

    let json = if json_pretty {
        serde_json::to_string_pretty(&field_defs)
    } else {
        serde_json::to_string(&field_defs)
    };
    let json = match json {
        Ok(s) => s,
        Err(e) => return Err(format!("Unable to serialize metadata to JSON: {}", e.to_string())),
    };
    match out_file.write(json.as_bytes()) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Unable to write serialized metadata: {}", e.to_string())),
    }
}

pub fn import_metadata_from_file(in_file: &mut std::fs::File) -> Result<EventDefinitions, String> {
    info!("Importing metadata from file");
    let mut buf_read = std::io::BufReader::new(in_file);
    let field_defs : EventDefinitions = match serde_json::from_reader(&mut buf_read) {
        Ok(v) => v,
        Err(e) => return Err(format!("Cannot deserialize JSON metadata from file: {}", e.to_string())),
    };
    Ok(field_defs)
}
