use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use crate::RenderingConfig;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EventFieldDefinition {
    pub name: String,
    pub out_type: String,
}

// Aliased type for ProviderName -> EventID -> Version -> FieldNumber -> EventFieldDefinition map
pub type EventFieldMapping = BTreeMap<String, BTreeMap<u64, BTreeMap<u64, BTreeMap<u64, EventFieldDefinition>>>>;

pub fn read_field_defs_from_system() -> Result<EventFieldMapping, String> {
    let mut field_defs = BTreeMap::new();

    info!("Importing event definitions from live system");

    for provider_name in crate::windows::get_evt_provider_names()? {
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

pub fn update_field_defs_with(known_defs: &mut EventFieldMapping, new_defs: &EventFieldMapping) {
    for (provider_name, new_events) in new_defs {
        let known_events = known_defs.entry(provider_name.to_owned()).or_insert(BTreeMap::new());
        for (eventid, new_versions) in new_events {
            let known_versions = known_events.entry(eventid.to_owned()).or_insert(BTreeMap::new());
            for (version, new_fields) in new_versions {
                let known_fields = known_versions.entry(version.to_owned()).or_insert(BTreeMap::new());
                for (field_num, field_def) in new_fields {
                    known_fields.insert(field_num.to_owned(), field_def.to_owned());
                }
            }
        }
    }
}

pub fn export_field_defs(field_defs: &EventFieldMapping,
                     out_file: &mut dyn std::io::Write,
                     json_pretty: bool) -> Result<(), String> {

    let json = if json_pretty {
        serde_json::to_string_pretty(&field_defs)
    } else {
        serde_json::to_string(&field_defs)
    };
    let json = match json {
        Ok(s) => s,
        Err(e) => return Err(format!("Unable to serialize field definitions to JSON: {}", e.to_string())),
    };
    match out_file.write(json.as_bytes()) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Unable to write serialized field definitions: {}", e.to_string())),
    }
}

pub fn read_field_defs_from_file(in_file: &mut std::fs::File) -> Result<EventFieldMapping, String> {
    info!("Importing event definitions from file");
    let mut buf_read = std::io::BufReader::new(in_file);
    let field_defs : EventFieldMapping = match serde_json::from_reader(&mut buf_read) {
        Ok(v) => v,
        Err(e) => return Err(format!("Cannot deserialize JSON from file: {}", e.to_string())),
    };
    Ok(field_defs)
}

pub fn get_field_name(provider_name: &str, eventid: &u64, version: &u64, field_num: &u64, context: &RenderingConfig) -> EventFieldDefinition {
    if let Some(events) = context.field_defs.get(provider_name) {
        if let Some(versions) = events.get(eventid) {
            if let Some(fields) = versions.get(version) {
                if let Some(field_def) = fields.get(field_num) {
                    return field_def.to_owned();
                }
            }
        }
    }
    EventFieldDefinition {
        name: format!("field{}", field_num),
        out_type: "xs:string".to_owned(),
    }
}
