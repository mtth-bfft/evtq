use winapi::um::winevt::*;
use winapi::shared::winerror::ERROR_INSUFFICIENT_BUFFER;
use std::ptr::null_mut;
use std::result::Result;
use winapi::ctypes::c_void;
use crate::windows::{EvtHandle, get_win32_errcode};
use crate::{RenderingConfig, OutputColumn};
use crate::formatting::{unwrap_variant_contents, bytes_as_hexstring, format_utc_systemtime, CommonEventProperties, EvtVariant};
use crate::event_defs::get_field_name;

pub fn render_event_json(h_event: &EvtHandle, common_props: &CommonEventProperties, render_cfg: &RenderingConfig) -> Result<(), String> {
    let h_ctxuser = unsafe { EvtCreateRenderContext(0, null_mut(), EvtRenderContextUser) };
    if h_ctxuser.is_null() {
        return Err(format!("EvtCreateRenderContext(EvtRenderContextUser) failed with code {}", get_win32_errcode()));
    }
    let h_ctxuser = EvtHandle::from_raw(h_ctxuser)?;
    let mut buffer_len_req : u32 = 0;
    let mut props_count : u32 = 0;
    let res = unsafe {
        EvtRender(h_ctxuser.as_ptr(),
                  h_event.as_ptr(),
                  EvtRenderEventValues,
                  0,
                  null_mut(),
                  &mut buffer_len_req as *mut u32,
                  &mut props_count as *mut u32)
    };
    // res can be != 0 here if, even with a NULL buffer, if there are no event values to render
    let mut buffer : Vec<u8> = Vec::with_capacity(buffer_len_req as usize);
    if res == 0 && get_win32_errcode() != ERROR_INSUFFICIENT_BUFFER {
        return Err(format!("EvtRender(EvtRenderEventValues) failed with code {}", get_win32_errcode()));
    }
    else if res == 0 {
        let res = unsafe {
            EvtRender(h_ctxuser.as_ptr(),
                      h_event.as_ptr(),
                      EvtRenderEventValues,
                      buffer_len_req,
                      buffer.as_mut_ptr() as *mut c_void,
                      &mut buffer_len_req as *mut u32,
                      &mut props_count as *mut u32)
        };
        if res == 0 {
            return Err(format!("Rendering event values failed with code {}", get_win32_errcode()));
        }
    }

    let mut event_json = serde_json::Map::new();
    for column in &render_cfg.columns {
        match column {
            OutputColumn::Hostname => event_json.insert("hostname".to_owned(),
                      serde_json::value::Value::from(common_props.hostname.to_owned())),
            OutputColumn::RecordID => event_json.insert("recordid".to_owned(),
                      serde_json::value::Value::from(common_props.recordid)),
            OutputColumn::Timestamp => event_json.insert("timestamp".to_owned(),
                      serde_json::value::Value::from(format_utc_systemtime(&common_props.timestamp, &render_cfg.datefmt))),
            OutputColumn::Provider => event_json.insert("provider".to_owned(),
                  serde_json::value::Value::from(common_props.provider.to_owned())),
            OutputColumn::EventID => event_json.insert("eventid".to_owned(),
                  serde_json::value::Value::from(common_props.eventid)),
            OutputColumn::Version => event_json.insert("version".to_owned(),
                  serde_json::value::Value::from(common_props.version)),
            OutputColumn::EventSpecific(prop_num) => {
                if prop_num >= &props_count {
                    break; // silently truncate lines which reference non-existent fields
                }
                let buffer_offset = (*prop_num as usize) * std::mem::size_of::<EVT_VARIANT>();
                let field_def = get_field_name(&common_props.provider,
                    &common_props.eventid,
                    &common_props.version,
                    &(*prop_num as u64),
                    render_cfg
                );
                let prop : EVT_VARIANT = unsafe {
                    std::ptr::read(buffer.as_ptr().add(buffer_offset) as *const _)
                };

                let prop = unwrap_variant_contents(&prop, Some(&field_def.out_type))?;
                let json_value = match prop {
                    EvtVariant::Null => serde_json::value::Value::Null,
                    EvtVariant::String(s) => serde_json::value::Value::from(s),
                    EvtVariant::UInt(i) => serde_json::value::Value::from(i),
                    EvtVariant::Int(i) => serde_json::value::Value::from(i),
                    EvtVariant::Single(f) => serde_json::value::Value::from(f),
                    EvtVariant::Double(f) => serde_json::value::Value::from(f),
                    EvtVariant::Boolean(b) => serde_json::value::Value::from(b),
                    EvtVariant::Binary(s) => serde_json::value::Value::from(bytes_as_hexstring(&s)),
                    EvtVariant::DateTime(d) => serde_json::value::Value::from(
                        format_utc_systemtime(&d, &render_cfg.datefmt)),
                };
                event_json.insert(field_def.name, json_value)
            },
        };
    }

    let json = if render_cfg.json_pretty {
        serde_json::to_string_pretty(&event_json)
    } else {
        serde_json::to_string(&event_json)
    };
    let json = match json {
        Ok(s) => s,
        Err(e) => return Err(format!("JSON serialization failed: {}", e)),
    };
    match render_cfg.output_file.lock() {
        Ok(mut f) => {
            match f.write((json + "\n").as_bytes()) {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Unable to write serialized JSON to file: {:?}", e)),
            }
        },
        Err(e) => return Err(format!("Failed to acquire lock to output file: {}", e)),
    }
}