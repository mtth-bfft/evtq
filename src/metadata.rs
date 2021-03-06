use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use crate::windows::{get_evt_provider_handle, get_evt_provider_metadata, format_message};
use winapi::shared::winerror::{ERROR_EVT_MESSAGE_NOT_FOUND, ERROR_EVT_MESSAGE_LOCALE_NOT_FOUND};
use winapi::um::winevt::{
    EvtPublisherMetadataPublisherGuid,
    EvtPublisherMetadataResourceFilePath,
    EvtPublisherMetadataParameterFilePath,
    EvtPublisherMetadataMessageFilePath,
    EvtPublisherMetadataPublisherMessageID,
};
use crate::formatting::EvtVariant;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EventFieldDefinition {
    pub name: String,
    pub out_type: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EventDefinition {
    pub channel: Option<String>,
    pub message: Option<String>,
    pub level: u32,
    pub level_name: Option<String>,
    pub opcode: u32,
    pub opcode_name: Option<String>,
    pub task: u32,
    pub task_name: Option<String>,
    pub keywords: u64,
    pub keyword_names: Vec<String>,
    pub fields: Vec<EventFieldDefinition>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProviderMetadata {
    pub guid: Option<String>,
    pub resource_file_path: Option<String>,
    pub parameter_file_path: Option<String>,
    pub message_file_path: Option<String>,
    pub message: Option<String>,
    pub events: BTreeMap<u64, BTreeMap<u64, EventDefinition>>,
}

pub type Metadata = BTreeMap<String, ProviderMetadata>;

pub fn import_metadata_from_system() -> Result<Metadata, String> {
    let mut metadata = BTreeMap::new();

    info!("Importing metadata from live system, this may take a while...
       (use --no-system-metadata if you don't care about message strings, field names
        and types, or use --export-metadata then --import-metadata to only do it once)");

    for provider_name in crate::windows::get_evt_provider_names()? {
        verbose!("Querying provider {}", provider_name);
        let (h_provmeta, h_evtenum) = match get_evt_provider_handle(&provider_name) {
            Ok(Some(handles)) => handles,
            Ok(None) => continue,
            Err(e) => {
                warn!("Unable to open handle to provider {} : {}", provider_name, e);
                continue;
            },
        };
        let events = match crate::windows::get_evt_provider_events(&provider_name, &h_provmeta, &h_evtenum) {
            Ok(map) => map,
            Err(e) => {
                warn!("Unable to enumerate events from provider '{}': error {}",
                          provider_name, e);
                continue;
            },
        };
        let guid = match get_evt_provider_metadata(&h_provmeta, EvtPublisherMetadataPublisherGuid) {
            Ok(EvtVariant::String(s)) => Some(s),
            Ok(o) => { warn!("Unexpected type for provider {} GUID: {:?}", provider_name, o); None },
            Err(e) => { warn!("Unable to query provider {} GUID: {}", provider_name, e); None },
        };
        let resource_file_path = match get_evt_provider_metadata(&h_provmeta, EvtPublisherMetadataResourceFilePath) {
            Ok(EvtVariant::String(s)) => Some(s),
            // Docs say EvtPublisherMetadataResourceFilePath is a String, actually it can also be Null
            Ok(EvtVariant::Null) => None,
            Ok(o) => { warn!("Unexpected type for provider {} resource filepath: {:?}", provider_name, o); None },
            Err(e) => { warn!("Unable to query provider {} resource filepath: {}", provider_name, e); None },
        };
        let parameter_file_path = match get_evt_provider_metadata(&h_provmeta, EvtPublisherMetadataParameterFilePath) {
            Ok(EvtVariant::String(s)) => Some(s),
            // Docs say EvtPublisherMetadataParameterFilePath is a String, actually it can also be Null
            Ok(EvtVariant::Null) => None,
            Ok(o) => { warn!("Unexpected type for provider {} parameter filepath: {:?}", provider_name, o); None },
            Err(e) => { warn!("Unable to query provider {} parameter filepath: {}", provider_name, e); None },
        };
        let message_file_path = match get_evt_provider_metadata(&h_provmeta, EvtPublisherMetadataMessageFilePath) {
            Ok(EvtVariant::String(s)) => Some(s),
            // Docs say EvtPublisherMetadataMessageFilePath is a String, actually it can also be Null
            Ok(EvtVariant::Null) => None,
            Ok(o) => { warn!("Unexpected type for provider {} message filepath: {:?}", provider_name, o); None },
            Err(e) => { warn!("Unable to query provider {} message filepath: {}", provider_name, e); None },
        };
        let message = match get_evt_provider_metadata(&h_provmeta, EvtPublisherMetadataPublisherMessageID) {
            // MessageID = (DWORD)(-1) if there's no message
            Ok(EvtVariant::UInt(4294967295)) => None,
            Ok(EvtVariant::UInt(message_id)) if message_id <= u32::MAX as u64 => {
                match format_message(&h_provmeta, message_id as u32) {
                    Ok(s) => Some(s),
                    Err((_, n)) if n == ERROR_EVT_MESSAGE_NOT_FOUND || n == ERROR_EVT_MESSAGE_LOCALE_NOT_FOUND => None,
                    Err((e, _)) => {
                        warn!("Unable to format provider {} message: {:?}", provider_name, e);
                        None
                    }
                }
            },
            Ok(o) => {
                warn!("Unexpected type for provider {} message ID: {:?}", provider_name, o);
                None
            },
            Err(e) => {
                warn!("Unable to query provider {} message ID: {}", provider_name, e);
                None
            },
        };

        metadata.insert(provider_name, ProviderMetadata {
            guid,
            resource_file_path,
            parameter_file_path,
            message_file_path,
            message,
            events,
        });
    }
    info!("Metadata import successful.");
    Ok(metadata)
}

pub fn update_metadata_with(known_meta: &mut Metadata, new_meta: &Metadata) {
    for (provider_name, new_prov_meta) in new_meta {
        let known_prov_meta = known_meta.entry(provider_name.to_owned()).or_insert(new_prov_meta.to_owned());
        if let Some(guid) = &new_prov_meta.guid {
            known_prov_meta.guid = Some(guid.to_owned());
        }
        for (eventid, new_versions) in &new_prov_meta.events {
            let known_versions = known_prov_meta.events.entry(eventid.to_owned()).or_insert(BTreeMap::new());
            for (version, new_def) in new_versions {
                match known_versions.get_mut(version) {
                    // If we didn't know anything about that event, use it, it can't be worse
                    None => { known_versions.insert(version.to_owned(), new_def.to_owned()); },
                    // We knew about this event, use new_meta to fill the blanks
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

pub fn export_metadata_to_file(metadata: &Metadata,
                               out_file: &mut dyn std::io::Write,
                               json_pretty: bool) -> Result<(), String> {

    let json = if json_pretty {
        serde_json::to_string_pretty(&metadata)
    } else {
        serde_json::to_string(&metadata)
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

pub fn import_metadata_from_file(in_file: &mut std::fs::File) -> Result<Metadata, String> {
    info!("Importing metadata from file");
    let mut buf_read = std::io::BufReader::new(in_file);
    let metadata : Metadata = match serde_json::from_reader(&mut buf_read) {
        Ok(v) => v,
        Err(e) => return Err(format!("Cannot deserialize JSON metadata from file: {}", e.to_string())),
    };
    Ok(metadata)
}
