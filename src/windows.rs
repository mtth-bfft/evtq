use std::ptr::{null_mut, NonNull};
use std::convert::TryFrom;
use std::collections::BTreeMap;
use std::time::Instant;
use std::ops::Deref;
use roxmltree;
use winapi::ctypes::c_void;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::minwinbase::SYSTEMTIME;
use std::sync::atomic::Ordering::Relaxed;
use winapi::shared::winerror::{ERROR_NO_MORE_ITEMS, ERROR_INVALID_OPERATION, ERROR_INSUFFICIENT_BUFFER, ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_RESOURCE_TYPE_NOT_FOUND, ERROR_INVALID_DATA, RPC_S_SERVER_UNAVAILABLE, ERROR_EVT_UNRESOLVED_VALUE_INSERT};
use winapi::um::winevt::*;
use crate::log::*;
use crate::RenderingConfig;
use crate::metadata::{EventFieldDefinition, EventDefinition};
use crate::formatting::{EvtVariant, CommonEventProperties, get_event_common_properties, unwrap_variant_contents};
use winapi::shared::minwindef::DWORD;
use std::str::FromStr;

const INFINITE : u32 = 0xFFFFFFFF;

// System-wide standard channels defined by Windows. Event-provider-specific channels
// are queried at runtime.
const SYSTEM_CHANNELS: &[(u32, &'static str, &'static str)] = &[
    (0, "TraceClassic", "Events for Classic ETW tracing"),
    (8, "System", "Events for all installed system services.  This channel is secured to applications running under system service accounts or user applications running under local adminstrator privileges"),
    (9, "Application", "Events for all user-level applications. This channel is not secured and open to any applications. Applications which log extensive information should define an application-specific channel"),
    (10, "Security", "The Windows Audit Log.  For exclusive use of the Windows Local Security Authority. User events may appear as audits if supported by the underlying application"),
    (11, "TraceLogging", "Event contains provider traits and TraceLogging event metadata"),
    (12, "ProviderMetadata", "Event contains provider traits"),
];

// System-wide standard levels defined by Windows. Event-provider-specific levels
// are queried at runtime.
const SYSTEM_LEVELS: &[(u32, &'static str, &'static str)] = &[
    (0, "win:LogAlways", "Log Always"),
    (1, "win:Critical", "Only critical errors"),
    (2, "win:Error", "All errors, includes win:Critical"),
    (3, "win:Warning", "All warnings, includes win:Error"),
    (4, "win:Informational", "All informational content, including win:Warning"),
    (5, "win:Verbose", "All tracing, including previous levels"),
];

// System-wide tasks defined by Windows. Event-provider-specific tasks
// are queried at runtime.
const SYSTEM_TASKS: &[(u32, &'static str, &'static str)] = &[
    (0, "win:None", "undefined task"),
];

// System-wide opcodes defined by Windows. Event-provider-specific opcodes
// are queried at runtime.
const SYSTEM_OPCODES: &[(u32, &'static str, &'static str)] = &[
    (0, "win:None", "An informational event"),
    (1, "win:Start", "An activity start event"),
    (2, "win:Stop", "An activity end event"),
    (3, "win:DC_Start", "A trace collection start event"),
    (4, "win:DC_Stop", "A trace collection end event"),
    (5, "win:Extension", "An extensional event"),
    (6, "win:Reply", "A reply event"),
    (7, "win:Resume", "An event representing the activity resuming from the suspension"),
    (8, "win:Suspend", "An event representing the activity is suspended, pending another activity's completion"),
    (9, "win:Send", "An event representing the activity is transferred to another component, and can continue to work"),
];

#[derive(Debug, PartialEq)]
enum EventFormatterState {
    LookingForEndOfUnformattedChunk { chunk_start_pos: usize },
    RightAfterPercentInUnformattedChunk { chunk_start_pos: usize },
    LookingForEndOfFormatNumber { number_start_pos: usize },
    LookingForEndOfFormatSpec,
    EndOfString,
}

#[derive(Debug)]
pub struct EvtHandle {
    handle: NonNull<c_void>,
    auto_free: bool,
}

pub struct RpcCredentials<'a> {
    pub domain: &'a str,
    pub username: &'a str,
    pub password: &'a str,
}

pub fn get_system_standard_val(arr: &'static [(u32, &'static str, &'static str)], val: u32) -> Option<(&'static str, &'static str)> {
    match arr.binary_search_by(|(k, _, _)| k.cmp(&val)) {
        Ok(i) => Some((&arr[i].1, &arr[i].2)),
        _ => None,
    }
}

impl EvtHandle {
    pub fn from_raw(handle: EVT_HANDLE) -> Result<EvtHandle, String> {
        match NonNull::new(handle) {
            Some(handle) => Ok(EvtHandle { handle: handle, auto_free: true }),
            None => Err(format!("Cannot use NULL EVT_HANDLE value")),
        }
    }

    pub fn from_raw_leak(handle: EVT_HANDLE) -> Result<EvtHandle, String> {
        match NonNull::new(handle) {
            Some(handle) => Ok(EvtHandle { handle: handle, auto_free: false }),
            None => Err(format!("Cannot use NULL EVT_HANDLE value")),
        }
    }

    pub fn as_ptr(&self) -> *mut c_void {
        self.handle.as_ptr()
    }
}

impl Drop for EvtHandle {
    fn drop(&mut self) {
        if !self.auto_free {
            return;
        }
        let res = unsafe { EvtClose(self.as_ptr()) };
        if res == 0 {
            warn!("EvtClose() failed with code {}", get_win32_errcode());
        }
    }
}

pub fn get_win32_errcode() -> u32 {
    unsafe { GetLastError() }
}

pub fn get_evt_publisher_enum_handle() -> Result<EvtHandle, String> {
    let handle = unsafe { EvtOpenPublisherEnum(null_mut(), 0) };
    if handle.is_null() {
        return Err(format!("EvtOpenPublisherEnum() failed with code {}", get_win32_errcode()));
    }
    EvtHandle::from_raw(handle)
}

pub fn get_evt_provider_handle(provider_name: &str) -> Result<Option<(EvtHandle, EvtHandle)>, String> {
    let mut buffer: Vec<u16> = provider_name.encode_utf16().collect();
    buffer.resize(buffer.len() + 1, 0); // append a terminating NULL character
    let h_metadata = unsafe {
        EvtOpenPublisherMetadata(null_mut(), buffer.as_ptr(), null_mut(), 0, 0)
    };
    if h_metadata.is_null() {
        match get_win32_errcode() {
            e if e == ERROR_FILE_NOT_FOUND || e == ERROR_RESOURCE_TYPE_NOT_FOUND || e == ERROR_INVALID_DATA => {
                verbose!("Discarding provider \"{}\" because EvtOpenPublisherMetadata() failed with code {}",
                         provider_name, e);
                return Ok(None);
            },
            other => return Err(format!("EvtOpenPublisherMetadata('{}') failed with code {}",
                                        provider_name, other)),
        }
    }
    let h_metadata = EvtHandle::from_raw(h_metadata)?;

    let h_evtenum = unsafe {
        EvtOpenEventMetadataEnum(h_metadata.as_ptr(), 0)
    };
    if h_evtenum.is_null() {
        match get_win32_errcode() {
            e if e == ERROR_RESOURCE_TYPE_NOT_FOUND => {
                verbose!("Discarding provider \"{}\" because EvtOpenEventMetadataEnum() failed with code {}",
                         provider_name, e);
                return Ok(None);
            },
            other => return Err(format!("EvtOpenEventMetadataEnum('{}') failed with code {}",
                                        provider_name, other)),
        }
    }
    let h_evtenum = EvtHandle::from_raw(h_evtenum)?;

    Ok(Some((h_metadata, h_evtenum)))
}

pub fn get_evt_provider_names() -> Result<Vec<String>, String> {
    let handle = get_evt_publisher_enum_handle()?;
    let mut result: Vec<String> = Vec::new();
    let mut buffer: Vec<u16> = Vec::new();
    let mut buffer_len_req: u32 = 0;

    loop {
        let res = unsafe {
            EvtNextPublisherId(handle.as_ptr(),
                               u32::try_from(buffer.capacity()).unwrap(),
                               buffer.as_mut_ptr(),
                               &mut buffer_len_req as *mut u32)
        };
        if res.eq(&0) {
            match get_win32_errcode() {
                ERROR_NO_MORE_ITEMS => break,
                ERROR_INSUFFICIENT_BUFFER => {
                    buffer.resize(usize::try_from(buffer_len_req).unwrap(), 0);
                    continue;
                },
                other => return Err(format!("EvtNextPublisherId() failed with code {}", other)),
            }
        }

        let slice = unsafe { std::slice::from_raw_parts(buffer.as_ptr(), buffer_len_req as usize - 1) };
        let provider_name = match String::from_utf16(slice) {
            Ok(s) => s,
            Err(e) => {
                warn!("Discarding provider '{}' which has non-unicode name: {}",
                          String::from_utf16_lossy(&buffer), e);
                continue;
            },
        };
        result.push(provider_name);
    }

    Ok(result)
}

pub fn get_evt_provider_metadata(h_provmeta: &EvtHandle, prop: EVT_PUBLISHER_METADATA_PROPERTY_ID) -> Result<EvtVariant, String> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut buffer_len_req: u32 = 0;
    let res = unsafe {
        EvtGetPublisherMetadataProperty(h_provmeta.as_ptr(), prop, 0, 0, null_mut(), &mut buffer_len_req)
    };
    if res != 0 || get_win32_errcode() != ERROR_INSUFFICIENT_BUFFER {
        return Err(format!("EvtGetPublisherMetadataProperty() failed with code {}", get_win32_errcode()));
    }
    buffer.resize(buffer_len_req as usize, 0);
    let res = unsafe {
        EvtGetPublisherMetadataProperty(h_provmeta.as_ptr(), prop, 0, buffer_len_req, buffer.as_mut_ptr() as PEVT_VARIANT, &mut buffer_len_req)
    };
    if res == 0 {
        return Err(format!("EvtGetPublisherMetadataProperty() failed with code {}", get_win32_errcode()));
    }
    let evt_variant : EVT_VARIANT = unsafe { std::ptr::read(buffer.as_ptr() as *const _) };
    unwrap_variant_contents(&evt_variant, None)
}

fn get_evt_metadata(h_evt: &EvtHandle, prop: EVT_EVENT_METADATA_PROPERTY_ID) -> Result<EvtVariant, String> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut buffer_len_req: u32 = 0;
    let res = unsafe {
        EvtGetEventMetadataProperty(h_evt.as_ptr(), prop, 0, 0, null_mut(), &mut buffer_len_req)
    };
    if res != 0 || get_win32_errcode() != ERROR_INSUFFICIENT_BUFFER {
        return Err(format!("EvtGetEventMetadataProperty() failed with code {}", get_win32_errcode()));
    }
    buffer.resize(buffer_len_req as usize, 0);
    let res = unsafe {
        EvtGetEventMetadataProperty(h_evt.as_ptr(), prop, 0, buffer_len_req, buffer.as_mut_ptr() as PEVT_VARIANT, &mut buffer_len_req)
    };
    if res == 0 {
        return Err(format!("EvtGetEventMetadataProperty() failed with code {}", get_win32_errcode()));
    }
    let evt_variant : EVT_VARIANT = unsafe { std::ptr::read(buffer.as_ptr() as *const _) };
    unwrap_variant_contents(&evt_variant, None)
}

fn get_evt_array_len(h_array: &EvtHandle) -> Result<DWORD, String> {
    let mut len: DWORD = 0;
    let res = unsafe { EvtGetObjectArraySize(h_array.as_ptr(),
                                             &mut len as *mut DWORD) };
    if res == 0 {
        return Err(format!("EvtGetObjectArraySize() failed with code {}", get_win32_errcode()));
    }
    Ok(len)
}

fn get_evt_array_prop(h_array: &EvtHandle, prop: EVT_PUBLISHER_METADATA_PROPERTY_ID, idx: DWORD) -> Result<EvtVariant, String> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut buffer_len_req: u32 = 0;
    let res = unsafe {
        EvtGetObjectArrayProperty(h_array.as_ptr(),
                                  prop,
                                  idx,
                                  0,
                                  0,
                                  null_mut(),
                                  &mut buffer_len_req as *mut u32)
    };
    if res != 0 || get_win32_errcode() != ERROR_INSUFFICIENT_BUFFER {
        return Err(format!("EvtGetObjectArrayProperty({}, {}) failed with code {}",
                           prop, idx, get_win32_errcode()));
    }
    buffer.resize(buffer_len_req as usize, 0);
    let res = unsafe {
        EvtGetObjectArrayProperty(h_array.as_ptr(),
                                  prop,
                                  idx,
                                  0,
                                  buffer_len_req,
                                  buffer.as_mut_ptr() as *mut _,
                                  &mut buffer_len_req as *mut u32)
    };
    if res == 0 {
        return Err(format!("EvtGetObjectArrayProperty({}, {}) failed with code {}",
                           prop, idx, get_win32_errcode()));
    }
    let evt_variant : EVT_VARIANT = unsafe { std::ptr::read(buffer.as_ptr() as *const _) };
    unwrap_variant_contents(&evt_variant, None)
}

pub fn get_evt_prov_metadata_mapping(h_provmeta: &EvtHandle,
                                     array_prop: EVT_PUBLISHER_METADATA_PROPERTY_ID,
                                     key_prop: EVT_PUBLISHER_METADATA_PROPERTY_ID,
                                     val_prop: EVT_PUBLISHER_METADATA_PROPERTY_ID,
) -> Result<BTreeMap<u64, String>, String> {

    let h_array = match get_evt_provider_metadata(h_provmeta, array_prop) {
        Ok(EvtVariant::Handle(h)) => h,
        Ok(_) => return Err(format!("Unexpected metadata value type returned for mapping type {}", array_prop)),
        Err(e) => return Err(e),
    };
    let mut mapping = BTreeMap::new();
    let num_vals = match get_evt_array_len(&h_array) {
        Ok(n) => n,
        Err(e) => {
            warn!("Unable to query provider-defined mapping type {} length: {}", array_prop, e);
            0
        },
    };
    for idx in 0..num_vals {
        let key = match get_evt_array_prop(&h_array, key_prop, idx) {
            Ok(EvtVariant::UInt(u)) => u,
            Ok(_) => {
                warn!("Unable to query some provider-defined mapping type {}: unexpected type returned as key", array_prop);
                break;
            }
            Err(e) => {
                warn!("Unable to query some provider-defined mapping type {}: {}", array_prop, e);
                break;
            },
        };
        let val = match get_evt_array_prop(&h_array, val_prop, idx) {
            Ok(EvtVariant::String(s)) => s,
            Ok(_) => {
                warn!("Unable to query some provider-defined mapping type {}: unexpected type returned as val", array_prop);
                break;
            }
            Err(e) => {
                warn!("Unable to query some provider-defined mapping type {}: {}", array_prop, e);
                break;
            },
        };
        mapping.insert(key, val);
    }
    Ok(mapping)
}

pub fn format_message(h_provmeta: &EvtHandle, message_id: u32) -> Result<String, String> {
    let mut buffer_req: DWORD = 0;
    let res = unsafe {
        EvtFormatMessage(h_provmeta.as_ptr(),
                         null_mut(),
                         message_id,
                         0,
                         null_mut() as *mut _,
                         EvtFormatMessageId,
                         0,
                         null_mut(),
                         &mut buffer_req as *mut _)
    };
    if res != 0 || get_win32_errcode() != ERROR_INSUFFICIENT_BUFFER {
        Err(format!("EvtFormatMessage({:?}, {}, EvtFormatMessageId) returned {} and error code {}",
                  h_provmeta, message_id, res, get_win32_errcode()))
    }
    else {
        let mut buf : Vec<u16> = Vec::new();
        buf.resize(buffer_req as usize, 0);
        let res = unsafe {
            EvtFormatMessage(h_provmeta.as_ptr(),
                             null_mut(),
                             message_id,
                             0,
                             null_mut() as *mut _,
                             EvtFormatMessageId,
                             buffer_req,
                             buf.as_mut_ptr(),
                             &mut buffer_req as *mut _)
        };
        // It's an error for EvtFormatMessage() to return a string with "%1" placeholders.
        // We don't care about placeholders, they're exactly what we want.
        if res == 0 && get_win32_errcode() != ERROR_EVT_UNRESOLVED_VALUE_INSERT {
            Err(format!("EvtFormatMessage({:?}, {}, EvtFormatMessageId) 2 returned {} and error code {}",
                  h_provmeta, message_id, res, get_win32_errcode()))
        }
        else {
            // Remove the NULL terminator and parse as UTF16
            buf.resize((buffer_req as usize) - 1, 0);
            Ok(String::from_utf16_lossy(buf.as_slice()))
        }
    }
}

// Returns a map from Event ID -> Version -> EventDefinition, or an error String
pub fn get_evt_provider_events(provider_name: &str,
                               h_provmeta: &EvtHandle,
                               h_evtenum: &EvtHandle,
) -> Result<BTreeMap<u64, BTreeMap<u64, EventDefinition>>, String>
{
    let mut result = BTreeMap::new();

    // Query all resolved level names of this provider, once
    let prov_levels = match get_evt_prov_metadata_mapping(&h_provmeta,
                                                     EvtPublisherMetadataLevels,
                                                     EvtPublisherMetadataLevelValue,
                                                     EvtPublisherMetadataLevelName) {
        Ok(map) => map,
        Err(e) => {
            warn!("Unable to query provider {} level names: {}", provider_name, e);
            BTreeMap::new()
        },
    };

    // Query all resolved opcode names of this provider, once
    let prov_opcodes = match get_evt_prov_metadata_mapping(&h_provmeta,
                                                     EvtPublisherMetadataOpcodes,
                                                     EvtPublisherMetadataOpcodeValue,
                                                     EvtPublisherMetadataOpcodeName) {
        Ok(map) => map,
        Err(e) => {
            warn!("Unable to query provider {} opcode names: {}", provider_name, e);
            BTreeMap::new()
        },
    };

    // Query all resolved task names of this provider, once
    let prov_tasks = match get_evt_prov_metadata_mapping(&h_provmeta,
                                                     EvtPublisherMetadataTasks,
                                                     EvtPublisherMetadataTaskValue,
                                                     EvtPublisherMetadataTaskName) {
        Ok(map) => map,
        Err(e) => {
            warn!("Unable to query provider {} task names: {}", provider_name, e);
            BTreeMap::new()
        },
    };

    // Query all resolved keyword names of this provider, once
    let prov_keywords = match get_evt_prov_metadata_mapping(&h_provmeta,
                                                     EvtPublisherMetadataKeywords,
                                                     EvtPublisherMetadataKeywordValue,
                                                     EvtPublisherMetadataKeywordName) {
        Ok(map) => map,
        Err(e) => {
            warn!("Unable to query provider {} keyword names: {}", provider_name, e);
            BTreeMap::new()
        },
    };

    // Query all channels of this provider, once
    let prov_channels = match get_evt_prov_metadata_mapping(&h_provmeta,
                                                     EvtPublisherMetadataChannelReferences,
                                                     EvtPublisherMetadataChannelReferenceID,
                                                     EvtPublisherMetadataChannelReferencePath) {
        Ok(map) => map,
        Err(e) => {
            warn!("Unable to query provider {} channel names: {}", provider_name, e);
            BTreeMap::new()
        },
    };

    loop {
        let h_evt = unsafe {
            EvtNextEventMetadata(h_evtenum.as_ptr(), 0)
        };
        if h_evt.is_null() {
            match get_win32_errcode() {
                ERROR_NO_MORE_ITEMS => break,
                e if e == ERROR_INVALID_DATA => {
                    verbose!("Discarding event field definitions because EvtNextEventMetadata('{}') failed with code {}", provider_name, e);
                    break;
                }
                other => return Err(format!("EvtNextEventMetadata('{}') failed with code {}",
                                            provider_name, other)),
            }
        }
        let h_evt = EvtHandle::from_raw(h_evt)?;
        let event_id = match get_evt_metadata(&h_evt, EventMetadataEventID) {
            Ok(EvtVariant::UInt(i)) => i,
            Ok(_) => return Err(format!("Unexpected value type returned for EventID")),
            Err(e) => return Err(e),
        };
        let version = match get_evt_metadata(&h_evt, EventMetadataEventVersion) {
            Ok(EvtVariant::UInt(i)) => i,
            Ok(_) => return Err(format!("Unexpected value type returned for EventVersion")),
            Err(e) => return Err(e),
        };
        let fields_template = match get_evt_metadata(&h_evt, EventMetadataEventTemplate) {
            Ok(EvtVariant::String(s)) => s,
            Ok(_) => return Err(format!("Unexpected value type returned for EventTemplate")),
            Err(e) => return Err(e),
        };
        let message_id = match get_evt_metadata(&h_evt, EventMetadataEventMessageID) {
            Ok(EvtVariant::UInt(id)) if id <= (std::u32::MAX as u64) => id,
            Ok(_) => return Err(format!("Unexpected value type returned for EventMessageID")),
            Err(e) => return Err(e),
        };
        let level = match get_evt_metadata(&h_evt, EventMetadataEventLevel) {
            Ok(EvtVariant::UInt(level)) if level <= u32::MAX as u64 => (level as u32),
            Ok(_) => return Err(format!("Unexpected value type returned for EventMetadataEventLevel")),
            Err(e) => return Err(e),
        };
        let opcode = match get_evt_metadata(&h_evt, EventMetadataEventOpcode) {
            Ok(EvtVariant::UInt(opcode)) if opcode <= u32::MAX as u64 => (opcode as u32),
            Ok(_) => return Err(format!("Unexpected value type returned for EventMetadataEventOpcode")),
            Err(e) => return Err(e),
        };
        let task = match get_evt_metadata(&h_evt, EventMetadataEventTask) {
            Ok(EvtVariant::UInt(task)) if task <= u32::MAX as u64 => (task as u32),
            Ok(_) => return Err(format!("Unexpected value type returned for EventMetadataEventTask")),
            Err(e) => return Err(e),
        };
        let keywords = match get_evt_metadata(&h_evt, EventMetadataEventKeyword) {
            Ok(EvtVariant::UInt(keywords)) => keywords,
            Ok(_) => return Err(format!("Unexpected value type returned for EventMetadataEventKeyword")),
            Err(e) => return Err(e),
        };
        let channel = match get_evt_metadata(&h_evt, EventMetadataEventChannel) {
            Ok(EvtVariant::UInt(n)) if n <= u32::MAX as u64 => (n as u32),
            Ok(_) => return Err(format!("Unexpected value type returned for EventMetadataEventChannel")),
            Err(e) => return Err(e),
        };

        let mut message: Option<String> = None;
        // message_id == (DWORD)(-1) means the provider doesn't have a message string for that event
        if message_id < 4294967295 {
            match format_message(h_provmeta, message_id as u32) {
                Ok(s) => { message = Some(s); },
                Err(e) => warn!("Unable to format event {}/{}/{} message: {}",
                    provider_name, event_id, version, e),
            }
        }

        // Parse field names from the template XML
        let mut fields : Vec<EventFieldDefinition> = Vec::new();
        if fields_template.len() > 0 {
            let xml = match roxmltree::Document::parse(&fields_template) {
                Ok(d) => d,
                Err(e) => {
                    warn!("Event {} #{} version {} : unable to parse XML template: {}",
                          provider_name, event_id, version, e);
                    continue;
                },
            };
            if !xml.root_element().has_tag_name("template") {
                warn!("Event {} #{} version {} has invalid XML template root node:\n{}",
                      provider_name, event_id, version, fields_template);
                continue;
            }
            for field_node in xml.root_element().children() {
                if !field_node.is_element() {
                    continue; // skip any comment
                }
                if !field_node.has_tag_name("data") {
                    if field_node.has_tag_name("struct") {
                        continue; // see issue #2
                    }
                    warn!("Event {} ID={} version={} has unexpected XML data node '{}':\n{}",
                          provider_name, event_id, version, field_node.tag_name().name(), fields_template);
                    break;
                };
                if let (Some(name), Some(out_type)) = (field_node.attribute("name"),
                                                       field_node.attribute("outType")) {
                    let field_def = EventFieldDefinition {
                        name: name.to_owned(),
                        out_type: out_type.to_owned(),
                    };
                    fields.push(field_def);
                } else {
                    warn!("Event {} #{} version {} has incomplete XML data node:\n{}",
                          provider_name, event_id, version, fields_template);
                    break;
                };
            }
        }

        // Resolve the channel ID to a name (the ID is useless otherwise)
        let channel = match prov_channels.get(&(channel as u64)) {
            Some(s) => Some(s.to_string()),
            None => {
                match get_system_standard_val(SYSTEM_CHANNELS, channel) {
                    Some((name, _)) => Some(name.to_string()),
                    None => {
                        if channel != 0 {
                            debug!("Event {}/{}/{} uses unknown channel ID {}",
                               provider_name, event_id, version, channel);
                        }
                        None
                    }
                }
            },
        };

        // Resolve the level u32 to a name
        let level_name = match prov_levels.get(&(level as u64)) {
            Some(s) => Some(s.to_string()),
            None => {
                match get_system_standard_val(SYSTEM_LEVELS, level) {
                    Some((name, _)) => Some(name.to_string()),
                    None => {
                        if level != 0 {
                            debug!("Undocumented level {} in {}/{}/{}", level, provider_name, event_id, version);
                        }
                        None
                    },
                }
            },
        };

        // Resolve the opcode u32 to a name
        let opcode_name = match prov_opcodes.get(&(opcode as u64)) {
            Some(s) => Some(s.to_string()),
            None => {
                match get_system_standard_val(SYSTEM_OPCODES, opcode) {
                    Some((name, _)) => Some(name.to_string()),
                    None => {
                        if opcode != 0 {
                            debug!("Undocumented opcode {} in {}/{}/{}", opcode, provider_name, event_id, version);
                        }
                        None
                    },
                }
            },
        };

        // Resolve the task u32 to a name
        let task_name = match prov_tasks.get(&(task as u64)) {
            Some(s) => Some(s.to_string()),
            None => {
                match get_system_standard_val(SYSTEM_TASKS, task) {
                    Some((name, _)) => Some(name.to_string()),
                    None => {
                        if task != 0 {
                            debug!("Undocumented task {} in {}/{}/{}", opcode, provider_name, event_id, task);
                        }
                        None
                    },
                }
            },
        };

        let mut keyword_names = Vec::new();
        // Ignore reserved placeholder bits 56-63, used to pass information about channels
        // (see winmeta.xml)
        for bit in 0..56 {
            let val = 1 << bit;
            if (val & keywords) == 0 {
                continue;
            }
            match prov_keywords.get(&(val as u64)) {
                Some(name) => keyword_names.push(name.to_string()),
                None => keyword_names.push(format!("0x{:X}", val)),
            }
        }

        // Insert everything into the final hashmap
        let versions = result.entry(event_id).or_insert(BTreeMap::new());
        if versions.contains_key(&version) {
            warn!("Event {} ID={} version={} enumerated more than once by EvtNextEventMetadata()",
                provider_name, event_id, version);
        }
        let event_def = EventDefinition {
            channel,
            message,
            level,
            level_name,
            opcode,
            opcode_name,
            task,
            task_name,
            keywords,
            keyword_names,
            fields,
        };
        let prev = versions.insert(version, event_def);
        if prev.is_some() {
            warn!("Event {} #{} version {} has more than one list of field definitions",
                provider_name, event_id, version);
        }
    }

    Ok(result)
}

pub fn open_evt_session(hostname: &str, credentials: Option<&RpcCredentials>) -> Result<EvtHandle, String> {
    let mut hostname_u16 : Vec<u16> = hostname.encode_utf16().collect();
    hostname_u16.resize(hostname_u16.len() + 1, 0); // NULL-terminate
    let mut domain_u16 : Vec<u16>;
    let mut username_u16 : Vec<u16>;
    let mut password_u16 : Vec<u16>;
    let mut rpc_creds = match credentials {
        Some(c) => {
            domain_u16 = c.domain.encode_utf16().collect();
            domain_u16.resize(domain_u16.len() + 1, 0); // NULL-terminate
            username_u16 = c.username.encode_utf16().collect();
            username_u16.resize(username_u16.len() + 1, 0); // NULL-terminate
            password_u16 = c.password.encode_utf16().collect();
            password_u16.resize(password_u16.len() + 1, 0); // NULL-terminate
            EVT_RPC_LOGIN {
                Server: hostname_u16.as_mut_ptr(),
                Domain: domain_u16.as_mut_ptr(),
                User: username_u16.as_mut_ptr(),
                Password: password_u16.as_mut_ptr(),
                Flags: EvtRpcLoginAuthNegotiate,
            }
        },
        None => {
            EVT_RPC_LOGIN {
                Server: hostname_u16.as_mut_ptr(),
                Domain: null_mut(),
                User: null_mut(),
                Password: null_mut(),
                Flags: EvtRpcLoginAuthNegotiate,
            }
        }
    };
    let h_session = unsafe { EvtOpenSession(EvtRpcLogin, &mut rpc_creds as *mut _ as *mut c_void, 0, 0) };
    if h_session.is_null() {
        return Err(format!("EvtOpenSession('{}') failed with code {}", hostname, get_win32_errcode()));
    }
    EvtHandle::from_raw(h_session)
}

pub fn evt_list_channels(session: &EvtHandle) -> Result<Vec<String>, String> {
    let mut h_enum = null_mut();
    for try_nb in 1..=5 {
        h_enum = unsafe { EvtOpenChannelEnum(session.as_ptr(), 0) };
        if h_enum.is_null() {
            match get_win32_errcode() {
                ERROR_ACCESS_DENIED => return Err(format!("Failed to enumerate channels: access denied. Are you admin? Is remote UAC enforced?")),
                RPC_S_SERVER_UNAVAILABLE if try_nb < 5 => {
                    verbose!("EvtOpenChannelEnum() failed with code RPC_S_SERVER_UNAVAILABLE, trying again...");
                    std::thread::sleep(std::time::Duration::from_millis(1000));
                    continue;
                },
                other => return Err(format!("EvtOpenChannelEnum() failed with code {}", other)),
            }
        }
    }
    let h_enum = EvtHandle::from_raw(h_enum)?;

    let mut result : Vec<String> = vec!();
    let mut buffer : Vec<u16> = vec!();
    let mut buffer_len_req : u32 = 0;
    loop {
        let res = unsafe { EvtNextChannelPath(h_enum.as_ptr(), buffer.capacity() as u32, buffer.as_mut_ptr(), &mut buffer_len_req as *mut u32) };
        if res == 0 || buffer_len_req == 0 {
            match get_win32_errcode() {
                ERROR_NO_MORE_ITEMS => break,
                ERROR_INSUFFICIENT_BUFFER => {
                    buffer.resize(buffer_len_req as usize, 0);
                    continue;
                },
                other => return Err(format!("EvtNextChannelPath() failed with code {}", other)),
            }
        }
        let slice = unsafe { std::slice::from_raw_parts(buffer.as_ptr(), (buffer_len_req - 1) as usize) };
        let channel_name = match String::from_utf16(slice) {
            Ok(s) => s,
            Err(e) => {
                warn!("Discarding channel '{}' which has non-unicode name: {}",
                          String::from_utf16_lossy(&buffer), e);
                continue;
            },
        };
        result.push(channel_name);
    }

    Ok(result)
}

pub fn evt_get_channel_type(session: &EvtHandle, channel_name: &str) -> Result<u32, String> {
    let mut channel_name_u16 : Vec<u16> = channel_name.encode_utf16().collect();
    channel_name_u16.resize(channel_name.len() + 1, 0); // append a NULL terminator
    let h_channel = unsafe { EvtOpenChannelConfig(session.as_ptr(), channel_name_u16.as_ptr(), 0) };
    if h_channel.is_null() {
        return Err(format!("EvtOpenChannelConfig('{}') failed with code {}",
                           channel_name, get_win32_errcode()));
    }
    let h_channel = EvtHandle::from_raw(h_channel)?;

    let mut buffer_len_req : u32 = 0;
    let res = unsafe { EvtGetChannelConfigProperty(h_channel.as_ptr(),
                                                   EvtChannelConfigType,
                                                   0,
                                                   0,
                                                   null_mut(),
                                                   &mut buffer_len_req)
    };
    if res != 0 || get_win32_errcode() != ERROR_INSUFFICIENT_BUFFER {
        return Err(format!("EvtGetChannelConfigProperty('{}') failed with code {}",
                   channel_name, get_win32_errcode()));
    }
    let mut buffer : Vec<u8> = vec!();
    buffer.resize(buffer_len_req as usize, 0);
    let res = unsafe { EvtGetChannelConfigProperty(h_channel.as_ptr(),
                                                   EvtChannelConfigType,
                                                   0,
                                                   buffer_len_req,
                                                   buffer.as_mut_ptr() as *mut EVT_VARIANT,
                                                   &mut buffer_len_req)
    };
    if res == 0 {
        return Err(format!("EvtGetChannelConfigProperty('{}') failed with code {}",
                   channel_name, get_win32_errcode()));
    }
    let res : EVT_VARIANT = unsafe { std::ptr::read(buffer.as_ptr() as *const _) };
    let res = match unwrap_variant_contents(&res, None) {
        Ok(v) => v,
        Err(e) => return Err(format!("EvtGetChannelConfigProperty('{}') returned invalid EVT_VARIANT: {}",
                                     channel_name, e)),
    };
    let res = match res {
        EvtVariant::UInt(i) => i,
        _ => return Err(format!("Unexpected EVT_VARIANT.Type returned from EvtGetChannelConfigProperty('{}')",
                                           channel_name)),
    };
    match u32::try_from(res) {
        Ok(i) => Ok(i),
        Err(e) => Err(format!("Unexpected high value {} returned from EvtGetChannelConfigProperty('{}'): {}",
                          res, channel_name, e.to_string()))
    }
}

pub fn can_channel_be_subscribed(session: &EvtHandle, channel_name: &str) -> Result<bool, String> {
    let channel_type = evt_get_channel_type(session, channel_name)?;
    return Ok(channel_type == EvtChannelTypeOperational || channel_type == EvtChannelTypeAdmin);
}

pub fn open_evt_backup(path: &str, xml_query: &Option<String>) -> Result<EvtHandle, String> {
    let mut xml_query_u16 : Vec<u16>;
    let mut xml_query_ptr: *const u16 = null_mut();
    if let Some(xml) = xml_query {
        xml_query_u16 = xml.encode_utf16().collect();
        xml_query_u16.resize(xml_query_u16.len() + 1, 0); // append a terminating NULL byte
        xml_query_ptr = xml_query_u16.as_ptr();
    }
    let mut path_u16 : Vec<u16> = path.encode_utf16().collect();
    path_u16.resize(path_u16.len() + 1, 0); // append a terminating NULL character
    let h_feed = unsafe { EvtQuery(null_mut(), path_u16.as_ptr(), xml_query_ptr, EvtQueryFilePath | EvtQueryForwardDirection) };
    if h_feed.is_null() {
        return Err(format!("EvtQuery('{}') failed with code {}", path, get_win32_errcode()));
    }
    EvtHandle::from_raw(h_feed)
}

pub extern "system" fn evt_render_callback(action: EVT_SUBSCRIBE_NOTIFY_ACTION, render_cfg: *mut c_void, handle: EVT_HANDLE) -> u32 {
    if action != EvtSubscribeActionDeliver {
        warn!("Error delivered instead of event object: cannot render this");
        return 0; // keep trying to render further events
    }
    // The h_event is freed by our caller. Don't EvtClose() it automatically. We just need
    // to wrap it in the common type accepted by our rendering functions
    let h_event = match EvtHandle::from_raw_leak(handle) {
        Err(e) => { warn!("Rendering callback called with invalid event handle: {}", e); return 0 },
        Ok(h) => h,
    };
    let render_cfg : Box<&RenderingConfig> = unsafe { Box::from_raw(render_cfg as *mut _) };
    if let Err(e) = render_event(&h_event, render_cfg.deref()) {
        warn!("Error during event rendering: {} ... resuming event dump", e);
    }

    // Prevent double-free of render_cfg reference
    Box::leak(render_cfg);

    return 0; // keep trying to render further events
}

pub fn subscribe_channel(h_session: &EvtHandle, channel_name: &str, render_cfg: &RenderingConfig, xml_query: &Option<String>, dump_existing: bool) -> Result<EvtHandle, String> {
    let mut channel_name_u16: Vec<u16> = channel_name.encode_utf16().collect();
    channel_name_u16.resize(channel_name_u16.len() + 1, 0); // NULL terminator
    let mut xml_query_u16 : Vec<u16>;
    let mut xml_query_ptr: *const u16 = null_mut();
    if let Some(ref xml) = xml_query {
        xml_query_u16 = xml.encode_utf16().collect();
        xml_query_u16.resize(xml_query_u16.len() + 1, 0); // append a terminating NULL byte
        xml_query_ptr = xml_query_u16.as_ptr();
    }
    let render_cfg = Box::into_raw(Box::from(render_cfg));
    let flags = if dump_existing {
        EvtSubscribeStartAtOldestRecord
    } else {
        EvtSubscribeToFutureEvents
    };
    let h_subscription = unsafe { EvtSubscribe(
        h_session.as_ptr(),
        null_mut(),
        channel_name_u16.as_ptr(), // the channel is useful, but only if xml_query is NULL
        xml_query_ptr,
        null_mut(),
        render_cfg as *mut c_void,
        Some(evt_render_callback),
        flags)
    } as *mut c_void;
    if h_subscription.is_null() {
        return Err(format!("EvtSubscribe('{}') failed with code {} when queried with filter:\n{:?}",
                           channel_name, get_win32_errcode(), xml_query));
    }
    let h_subscription = EvtHandle::from_raw(h_subscription)?;

    Ok(h_subscription)
}

pub fn synchronous_poll_all_events(h_feed: &EvtHandle, render_cfg: &RenderingConfig) -> Result<(), String> {
    let mut count_events : u32 = 0;
    loop {
        let mut h_event : EVT_HANDLE = null_mut();
        let res = unsafe { EvtNext(h_feed.as_ptr(), 1, &mut h_event as *mut *mut c_void, INFINITE, 0, &mut count_events as *mut u32) };
        if res == 0 {
            match get_win32_errcode() {
                // These two errors are returned by EvtNext when it's out of events to return
                ERROR_NO_MORE_ITEMS | ERROR_INVALID_OPERATION => {
                    break;
                },
                other => return Err(format!("EvtNext() failed with code {}", other)),
            }
        }
        let h_event = EvtHandle::from_raw(h_event)?;
        if let Err(e) = render_event(&h_event, render_cfg) {
            warn!("Error during rendering: {} ... resuming event dump", e);
            continue;
        }
    }
    Ok(())
}

fn debug_event(h_event: &EvtHandle, error: String) {
    if get_log_level() >= LOG_LEVEL_DEBUG {
        debug!(" [!] Event rendering failed: {}", error);
        let mut context = crate::RenderingConfig {
            render_callback: crate::xml::render_event_xml,
            output_file: Box::from(std::sync::Mutex::new(std::io::stderr())),
            datefmt: "".to_string(),
            metadata: BTreeMap::new(),
            field_separator: '\0',
            json_pretty: false,
            columns: vec![],
            rendering_start: std::time::Instant::now(),
            event_counter: std::sync::atomic::AtomicU64::new(0),
        };
        crate::xml::render_event_xml(h_event, &CommonEventProperties{
            timestamp: SYSTEMTIME {
                wYear: 0,
                wMonth: 0,
                wDayOfWeek: 0,
                wDay: 0,
                wHour: 0,
                wMinute: 0,
                wSecond: 0,
                wMilliseconds: 0
            },
            hostname: "".to_string(),
            recordid: 0,
            provider: "".to_string(),
            eventid: 0,
            version: 0
        }, &mut context).unwrap();
    }
}

pub fn render_event(h_event: &EvtHandle, render_cfg: &RenderingConfig) -> Result<(), String> {
    let common_props = match get_event_common_properties(&h_event) {
        Err(e) => {
            debug_event(h_event, format!("Common property formatting failed: {}", e));
            return Err(format!("Error occured during common property formatting: {}", e));
        }
        Ok(None) => return Ok(()),
        Ok(Some(props)) => props,
    };

    if let Err(e) = (render_cfg.render_callback)(h_event, &common_props, render_cfg) {
        debug_event(h_event, format!("Rendering function returned: {}", e));
        return Err(format!("Error occured during rendering: {}", e));
    }

    let rendered_events = render_cfg.event_counter.fetch_add(1, Relaxed);
    if rendered_events % 1000 == 0 {
        let elapsed = Instant::now().duration_since(render_cfg.rendering_start);
        debug!("{} events rendered ({:.2}/s)", rendered_events, (rendered_events as f64)/elapsed.as_secs_f64());
    }

    Ok(())
}

// This does not support %%N syntax (references to system-wide message IDs,
// see https://docs.microsoft.com/en-us/windows/win32/api/winevt/nf-winevt-evtformatmessage)
// because I didn't find a way to query them and include them in the event
// definition JSON dump. That shouldn't be a problem, since only one event
// in one useless provider seems to use that syntax it as of 1909.
// This also doesn't support format string like %3!S! on purpose, since the
// actual formatting and argument type is determined by our own formatting
// function (see comment inside).
pub fn format_event_message(event_def: &EventDefinition, variants: *const EVT_VARIANT, variant_count: u32) -> Result<String, String> {
    // We can't use EvtFormatMessage() because that would require holding a
    // handle to the metadata of the provider which generated that event,
    // and we must be able to format messages offline.
    // We can't use FormatMessage() either, which assumes that %1 means %1!s!
    // so it would require formatting all variants (SIDs, GUIDs, int, etc.)
    // to strings beforehand, and would probably conflict with the few events
    // which take care to define the format string they expect (e.g. %1!S!
    // would make FormatMessage() parse our wide-string-formatted-variant as
    // an ANSI string).
    // The format syntax is way more complicated than replace('%1', args[1]):
    // it supports the entire printf format specification
    // (e.g. %1!*.*s! %4 %5!*s!", see
    // https://docs.microsoft.com/fr-fr/windows/win32/api/winbase/nf-winbase-formatmessage )

    let template = match &event_def.message {
        Some(t) => t,
        None => return Err(format!("Cannot format event without template")),
    };

    // Cache for the result of each EVT_VARIANT formatting to string
    let mut formatted_variants: Vec<Option<String>> = vec![None; variant_count as usize];
    let mut res = String::new(); // the final returned String
    let mut state = EventFormatterState::LookingForEndOfUnformattedChunk { chunk_start_pos: 0 };
    for (pos, c) in template.char_indices().chain(vec![(template.len(), '\0')].into_iter()) {
        state = match (c, state) {
            ('%', EventFormatterState::LookingForEndOfUnformattedChunk { chunk_start_pos }) =>
                EventFormatterState::RightAfterPercentInUnformattedChunk { chunk_start_pos },
            ('%', EventFormatterState::RightAfterPercentInUnformattedChunk { chunk_start_pos }) =>
                EventFormatterState::LookingForEndOfUnformattedChunk { chunk_start_pos },
            (c, EventFormatterState::RightAfterPercentInUnformattedChunk { chunk_start_pos }) if c.is_digit(10) => {
                res.push_str(&template[chunk_start_pos..pos - 1]);
                EventFormatterState::LookingForEndOfFormatNumber { number_start_pos: pos }
            },
            (c, EventFormatterState::LookingForEndOfFormatNumber { number_start_pos }) if !c.is_digit(10) => {
                let fmt_num = match u32::from_str(&template[number_start_pos..pos]) {
                    Ok(fmt_num) => fmt_num,
                    Err(_) => return Err(format!("Unable to parse format argument number from \"{}\"",
                        &template[number_start_pos..pos])),
                };
                let fmt_idx = (fmt_num as usize) - 1;
                if fmt_num > variant_count {
                    return Err(format!("Format argument number out-of-range ({}, only {} variants)",
                        fmt_num, variant_count));
                }
                if formatted_variants[fmt_idx].is_none() {
                    let buffer_offset = fmt_idx * std::mem::size_of::<EVT_VARIANT>();
                    let prop : EVT_VARIANT = unsafe {
                        std::ptr::read((variants as *const u8).add(buffer_offset) as *const _)
                    };
                    let type_hint = if fmt_idx < event_def.fields.len() {
                        Some(&event_def.fields[fmt_idx].out_type[..])
                    } else {
                        None
                    };
                    let prop = unwrap_variant_contents(&prop, type_hint)?;
                    let str_to_insert = match prop {
                        EvtVariant::Null => "null".to_string(),
                        EvtVariant::Handle(_) => "<handle>".to_string(),
                        EvtVariant::String(s) => s,
                        EvtVariant::UInt(u) => format!("{}", u),
                        EvtVariant::Int(i) => format!("{}", i),
                        EvtVariant::Single(f) => format!("{}", f),
                        EvtVariant::Double(d) => format!("{}", d),
                        EvtVariant::Boolean(b) => (if b { "true" } else { "false" }).to_string(),
                        EvtVariant::Binary(v) => format!("{:?}", v),
                        EvtVariant::DateTime(d) => format!("{}-{}-{} {}:{}:{}.{}",
                                                           d.wYear, d.wMonth, d.wDay, d.wHour,
                                                           d.wMinute, d.wSecond, d.wMilliseconds),
                    };
                    formatted_variants[fmt_idx] = Some(str_to_insert);
                }
                res.push_str(formatted_variants[fmt_idx].as_ref().unwrap());
                if c == '!' {
                    EventFormatterState::LookingForEndOfFormatSpec
                } else if pos == template.len() {
                    EventFormatterState::EndOfString
                }
                else {
                    EventFormatterState::LookingForEndOfUnformattedChunk { chunk_start_pos: pos }
                }
            },
            ('!', EventFormatterState::LookingForEndOfFormatSpec) =>
                EventFormatterState::LookingForEndOfUnformattedChunk { chunk_start_pos: pos + 1 },
            ('\0', EventFormatterState::LookingForEndOfUnformattedChunk { chunk_start_pos }) if pos == template.len() => {
                res.push_str(&template[chunk_start_pos..pos]);
                EventFormatterState::EndOfString
            },
            (_, state) => state,
        };
    }
    if state != EventFormatterState::EndOfString {
        return Err(format!("Unexpected final parser state {:?}", state));
    }
    Ok(res)
}