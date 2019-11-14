use winapi::um::winevt::*;
use winapi::shared::winerror::ERROR_INSUFFICIENT_BUFFER;
use std::ptr::null_mut;
use std::result::Result;
use winapi::ctypes::c_void;
use crate::windows::{EvtHandle, get_win32_errcode};
use crate::formatting::CommonEventProperties;
use crate::RenderingConfig;

pub fn render_event_xml(h_event: &EvtHandle, _common_props: &CommonEventProperties, render_cfg: &RenderingConfig) -> Result<(), String> {
    let mut buffer_len_req : u32 = 0;
    let mut unused : u32 = 0;
    let res = unsafe {
        EvtRender(null_mut(),
                  h_event.as_ptr(),
                  EvtRenderEventXml,
                  0,
                  null_mut(),
                  &mut buffer_len_req as *mut u32,
                  &mut unused as *mut u32)
    };
    if res != 0 || get_win32_errcode() != ERROR_INSUFFICIENT_BUFFER {
        return Err(format!("EvtRender() failed with code {}", get_win32_errcode()));
    }
    let mut buffer : Vec<u16> = Vec::with_capacity(((buffer_len_req + 1) / 2 + 1) as usize);
    let res = unsafe {
        EvtRender(null_mut(),
                  h_event.as_ptr(),
                  EvtRenderEventXml,
                  buffer_len_req,
                  buffer.as_mut_ptr() as *mut c_void,
                  &mut buffer_len_req as *mut u32,
                  &mut unused as *mut u32)
    };
    if res == 0 {
        return Err(format!("Event rendering as XML failed with code {}", get_win32_errcode()));
    }
    let slice = unsafe { std::slice::from_raw_parts(buffer.as_ptr(), ((buffer_len_req + 1) / 2) as usize) };
    let xml = match String::from_utf16(slice) {
        Ok(s) => s,
        Err(e) => {
            return Err(format!("Discarding event with non-unicode XML rendering ({}): {}",
                      e, String::from_utf16_lossy(&buffer)));
        },
    };

    match render_cfg.output_file.lock() {
        Ok(mut f) => {
            match f.write((xml + "\n").as_bytes()) {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Unable to write XML to file: {:?}", e)),
            }
        },
        Err(e) => return Err(format!("Failed to acquire lock to output file: {}", e)),
    }
}