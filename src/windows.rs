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
use winapi::shared::winerror::{
    ERROR_NO_MORE_ITEMS,
    ERROR_INVALID_OPERATION,
    ERROR_INSUFFICIENT_BUFFER,
    ERROR_ACCESS_DENIED,
    ERROR_FILE_NOT_FOUND,
    ERROR_RESOURCE_TYPE_NOT_FOUND,
    ERROR_INVALID_DATA,
    RPC_S_SERVER_UNAVAILABLE,
};
use winapi::um::winevt::*;
use crate::log::*;
use crate::RenderingConfig;
use crate::event_defs::EventFieldDefinition;
use crate::formatting::{EvtVariant, CommonEventProperties, get_event_common_properties, unwrap_variant_contents};

const INFINITE : u32 = 0xFFFFFFFF;

pub struct EvtHandle {
    handle: NonNull<c_void>,
    auto_free: bool,
}

pub struct RpcCredentials<'a> {
    pub domain: &'a str,
    pub username: &'a str,
    pub password: &'a str,
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
        EvtGetEventMetadataProperty(h_evt.as_ptr(), prop, 0, buffer.capacity() as u32, buffer.as_mut_ptr() as PEVT_VARIANT, &mut buffer_len_req)
    };
    if res == 0 {
        return Err(format!("EvtGetEventMetadataProperty() failed with code {}", get_win32_errcode()));
    }
    let evt_variant : EVT_VARIANT = unsafe { std::ptr::read(buffer.as_ptr() as *const _) };
    unwrap_variant_contents(&evt_variant, None)
}

pub fn get_evt_provider_event_fields(provider_name: &str) -> Result<BTreeMap<u64, BTreeMap<u64, BTreeMap<u64, EventFieldDefinition>>>, String> {
    let mut result = BTreeMap::new();
    let (_h_metadata, h_evtenum) = match get_evt_provider_handle(provider_name)? {
        Some(handles) => handles,
        None => return Ok(result),
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

        if fields_template.len() == 0 {
            continue;
        }

        let versions = result.entry(event_id).or_insert(BTreeMap::new());
        let fields = versions.entry(version).or_insert(BTreeMap::new());

        if fields.len() > 0 {
            warn!("Event {} #{} version {} has more than one list of field definitions",
                provider_name, event_id, version);
        }

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

        // Parse field names from the template XML
        let mut field_num : u64 = 0;
        for field_node in xml.root_element().children() {
            if !field_node.is_element() {
                continue; // skip any comment
            }
            if !field_node.has_tag_name("data") {
                if field_node.has_tag_name("struct") {
                    continue; // see issue #2
                }
                warn!("Event {} #{} version {} has unexpected XML data node '{}':\n{}",
                          provider_name, event_id, version, field_node.tag_name().name(), fields_template);
                break;
            };
            if let (Some(name), Some(out_type)) = (field_node.attribute("name"),
                                                   field_node.attribute("outType")) {
                let field_def = EventFieldDefinition {
                    name: name.to_owned(),
                    out_type: out_type.to_owned(),
                };
                fields.insert(field_num as u64, field_def);
                field_num += 1;
            } else {
                warn!("Event {} #{} version {} has incomplete XML data node:\n{}",
                          provider_name, event_id, version, fields_template);
                break;
            };
        }
    }

    Ok(result)
}

pub fn open_evt_session(hostname: &str, credentials: Option<&RpcCredentials>) -> Result<EvtHandle, String> {
    let mut hostname_u16 : Vec<u16> = hostname.encode_utf16().collect();
    let mut domain_u16 : Vec<u16>;
    let mut username_u16 : Vec<u16>;
    let mut password_u16 : Vec<u16>;
    let mut rpc_creds = match credentials {
        Some(c) => {
            domain_u16 = c.domain.encode_utf16().collect();
            username_u16 = c.username.encode_utf16().collect();
            password_u16 = c.password.encode_utf16().collect();
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
            field_defs: BTreeMap::new(),
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