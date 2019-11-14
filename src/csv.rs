use winapi::um::winevt::*;
use winapi::shared::winerror::ERROR_INSUFFICIENT_BUFFER;
use std::ptr::null_mut;
use std::result::Result;
use winapi::ctypes::c_void;
use crate::windows::{EvtHandle, get_win32_errcode};
use crate::{RenderingConfig, OutputColumn};
use crate::formatting::{unwrap_variant_contents, bytes_as_hexstring, format_utc_systemtime, CommonEventProperties, EvtVariant};

fn push_filtered_str(dest: &mut String, append: &str, forbidden: &char) {
    dest.push_str(&append.replace(&forbidden.to_string(), " "))
}

pub fn render_event_csv(h_event: &EvtHandle, common_props: &CommonEventProperties, render_cfg: &RenderingConfig) -> Result<(), String> {
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
    let mut buffer: Vec<u8> = Vec::with_capacity(buffer_len_req as usize);
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
            return Err(format!("EvtRender(EvtRenderEventValues) failed with code {}", get_win32_errcode()));
        }
    }

    let mut line = String::new();
    let mut first = true;
    for column in &render_cfg.columns {
        if ! first {
            line.push(render_cfg.field_separator);
        }
        first = false;
        match column {
            OutputColumn::Hostname => push_filtered_str(&mut line,
                                                        &common_props.hostname,
                                                        &render_cfg.field_separator),
            OutputColumn::RecordID => push_filtered_str(&mut line,
                                                        &common_props.recordid.to_string(),
                                                        &render_cfg.field_separator),
            OutputColumn::Timestamp => push_filtered_str(&mut line,
                                                         &format_utc_systemtime(&common_props.timestamp, &render_cfg.datefmt),
                                                         &render_cfg.field_separator),
            OutputColumn::Provider => push_filtered_str(&mut line,
                                                        &common_props.provider,
                                                        &render_cfg.field_separator),
            OutputColumn::EventID => push_filtered_str(&mut line,
                                                       &common_props.eventid.to_string(),
                                                       &render_cfg.field_separator),
            OutputColumn::Version => push_filtered_str(&mut line,
                                                       &common_props.version.to_string(),
                                                       &render_cfg.field_separator),
            OutputColumn::EventSpecific(prop_num) => {
                if prop_num >= &props_count {
                    break; // silently truncate lines which reference non-existent fields
                }
                let buffer_offset = (*prop_num as usize) * std::mem::size_of::<EVT_VARIANT>();
                let prop : EVT_VARIANT = unsafe {
                    std::ptr::read(buffer.as_ptr().add(buffer_offset) as *const _)
                };
                let prop = unwrap_variant_contents(&prop, None)?;
                match prop {
                    EvtVariant::Null => (),
                    EvtVariant::String(s) => push_filtered_str(&mut line,
                                                                      &s,
                                                                      &render_cfg.field_separator),
                    EvtVariant::UInt(i) => push_filtered_str(&mut line,
                                                                   &i.to_string(),
                                                                   &render_cfg.field_separator),
                    EvtVariant::Int(i) => push_filtered_str(&mut line,
                                                                 &i.to_string(),
                                                                 &render_cfg.field_separator),
                    EvtVariant::Single(f) => push_filtered_str(&mut line,
                                                                    &f.to_string(),
                                                                    &render_cfg.field_separator),
                    EvtVariant::Double(f) => push_filtered_str(&mut line,
                                                                     &f.to_string(),
                                                           &render_cfg.field_separator),
                    EvtVariant::Boolean(b) => push_filtered_str(&mut line,
                                                                      if b { "true" } else { "false" },
                                                                       &render_cfg.field_separator),
                    EvtVariant::Binary(s) => push_filtered_str(&mut line,
                                                                        &bytes_as_hexstring(&s),
                                                                        &render_cfg.field_separator),
                    EvtVariant::DateTime(d) => push_filtered_str(&mut line,
                                                                                &format_utc_systemtime(&d, &render_cfg.datefmt),
                                                                                &render_cfg.field_separator),
                }
            },
        };
    }
    line.push('\n');

    match render_cfg.output_file.lock() {
        Ok(mut f) => {
            match f.write(line.as_bytes()) {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Unable to write line to output file: {:?}", e)),
            }
        },
        Err(e) => return Err(format!("Failed to acquire lock to output file: {}", e)),
    }
}