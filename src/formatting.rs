use winapi::um::winevt::*;
use winapi::um::minwinbase::SYSTEMTIME;
use winapi::um::winbase::LocalFree;
use winapi::um::timezoneapi::FileTimeToSystemTime;
use winapi::shared::sddl::ConvertSidToStringSidW;
use winapi::shared::guiddef::GUID;
use std::ptr::null_mut;
use winapi::ctypes::c_void;
use crate::windows::{EvtHandle, get_win32_errcode};
use winapi::shared::winerror::ERROR_INSUFFICIENT_BUFFER;
use winapi::shared::minwindef::FILETIME;
use std::fmt::Debug;
use winapi::_core::fmt::Formatter;

pub struct CommonEventProperties {
    pub timestamp: SYSTEMTIME,
    pub hostname: String,
    pub recordid: u64,
    pub provider: String,
    pub eventid: u64,
    pub version: u64,
}

pub enum EvtVariant {
    Null,
    String(String),
    Handle(EvtHandle),
    UInt(u64),
    Int(i64),
    Single(f32),
    Double(f64),
    Boolean(bool),
    Binary(Vec<u8>),
    DateTime(SYSTEMTIME),
}

impl Debug for EvtVariant {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EvtVariant::Null => write!(f, "EvtVariant::Null"),
            EvtVariant::String(x) => write!(f, "EvtVariant::String(\"{}\")", x),
            EvtVariant::Handle(x) => write!(f, "EvtVariant::Handle({:?})", x),
            EvtVariant::UInt(x) => write!(f, "EvtVariant::Handle({})", x),
            EvtVariant::Int(x) => write!(f, "EvtVariant::Handle({})", x),
            EvtVariant::Single(x) => write!(f, "EvtVariant::Handle({})", x),
            EvtVariant::Double(x) => write!(f, "EvtVariant::Handle({})", x),
            EvtVariant::Boolean(x) => write!(f, "EvtVariant::Handle({})", x),
            EvtVariant::Binary(x) => write!(f, "EvtVariant::Handle({:?})", x),
            EvtVariant::DateTime(x) => write!(f,
                "EvtVariant::DateTime({}-{}-{} {}:{}:{}.{})",
                x.wYear, x.wMonth, x.wDay, x.wHour, x.wMinute, x.wSecond, x.wMilliseconds
            ),
        }
    }
}

pub fn bytes_as_hexstring(bytes: &[u8]) -> String {
    let mut res= String::new();
    for byte in bytes {
        res.push_str(&format!("{:02x}", byte));
    }
    res
}

pub fn hexstring_to_uint(hex: &str) -> Option<u64> {
    let hex = hex.to_lowercase().replace(" ", "").replace("0x", "");
    match u64::from_str_radix(&hex, 16) {
        Ok(u) => Some(u),
        _ => None
    }
}

pub fn format_utc_systemtime(stime: &SYSTEMTIME, datefmt: &str) -> String {
    let res = datefmt.to_owned();
    let res = res.replace("%Y", &format!("{:04}", stime.wYear));
    let res = res.replace("%m", &format!("{:02}", stime.wMonth));
    let res = res.replace("%d", &format!("{:02}", stime.wDay));
    let res = res.replace("%H", &format!("{:02}", stime.wHour));
    let res = res.replace("%M", &format!("{:02}", stime.wMinute));
    let res = res.replace("%S", &format!("{:02}", stime.wSecond));
    let res = res.replace("%.3f", &format!(".{:03}", stime.wMilliseconds));
    let res = res.replace("%z", "+0000");
    res
}

pub fn unwrap_variant_contents(variant: &EVT_VARIANT, type_hint: Option<&str>) -> Result<EvtVariant, String> {
    // Arrays are treated recursively, rendered as "[" + str(value1) + "," + str(value2) + ...
    if (variant.Type & EVT_VARIANT_TYPE_ARRAY) == EVT_VARIANT_TYPE_ARRAY {
        return Ok(EvtVariant::String(format!("[array]")));
        // TODO: recursive formatting, but requires unsafe pointer arithmetics inside EVT_VARIANT...
    }
    let res = match variant.Type {
        EvtVarTypeNull => EvtVariant::Null,
        EvtVarTypeString => {
            // NULL-terminated UTF-16 string
            let slice : &[u16];
            unsafe {
                let ptr = variant.u.StringVal();
                let len = (0..).take_while(|&i| *ptr.offset(i) != 0).count();
                slice = std::slice::from_raw_parts(*ptr, len);
            }
            match String::from_utf16(slice) {
                Ok(s) => EvtVariant::String(s),
                Err(e) => return Err(
                    format!("Cannot unwrap EVT_VARIANT: UTF16 conversion error: {}", e.to_string())),
            }
        },
        EvtVarTypeAnsiString => {
            // NULL-terminated UTF-8 string
            let slice : &[u8];
            unsafe {
                let ptr = variant.u.AnsiStringVal();
                let len = (0..).take_while(|&i| *ptr.offset(i) != 0).count();
                slice = std::slice::from_raw_parts(*ptr as *const u8, len);
            }
            match String::from_utf8(slice.to_vec()) {
                Ok(s) => EvtVariant::String(s),
                Err(e) => return Err(
                    format!("Cannot unwrap EVT_VARIANT: UTF8 conversion error: {}", e.to_string())),
            }
        },
        EvtVarTypeSByte => {
            let val : &i8 = unsafe { variant.u.SByteVal() };
            EvtVariant::Int(*val as i64)
        },
        EvtVarTypeByte => {
            let val : &u8 = unsafe { variant.u.ByteVal() };
            EvtVariant::UInt(*val as u64)
        },
        EvtVarTypeInt16 => {
            let val : &i16 = unsafe { variant.u.Int16Val() };
            EvtVariant::Int(*val as i64)
        },
        EvtVarTypeUInt16 => {
            let val : &u16 = unsafe { variant.u.UInt16Val() };
            EvtVariant::UInt(*val as u64)
        },
        EvtVarTypeInt32 => {
            let val : &i32 = unsafe { variant.u.Int32Val() };
            EvtVariant::Int(*val as i64)
        },
        EvtVarTypeUInt32 => {
            let val : &u32 = unsafe { variant.u.UInt32Val() };
            EvtVariant::UInt(*val as u64)
        },
        EvtVarTypeInt64 => {
            let val : &i64 = unsafe { variant.u.Int64Val() };
            EvtVariant::Int(*val as i64)
        },
        EvtVarTypeUInt64 => {
            let val : &u64 = unsafe { variant.u.UInt64Val() };
            EvtVariant::UInt(*val as u64)
        },
        EvtVarTypeSingle => {
            let val : &f32 = unsafe { variant.u.SingleVal() };
            EvtVariant::Single(*val)
        },
        EvtVarTypeDouble => {
            let val : &f64 = unsafe { variant.u.DoubleVal() };
            EvtVariant::Double(*val)
        },
        EvtVarTypeBoolean => {
            let val : &i32 = unsafe { variant.u.BooleanVal() };
            EvtVariant::Boolean(*val != 0)
        },
        EvtVarTypeBinary => {
            let val : u8 = unsafe { **variant.u.BinaryVal() };
            EvtVariant::String(format!("{:02X}", val))
        },
        EvtVarTypeGuid => {
            let val : GUID = unsafe { std::ptr::read(*variant.u.GuidVal()) };
            EvtVariant::String(format!(
                "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
                val.Data1, val.Data2, val.Data3, val.Data4[0], val.Data4[1], val.Data4[2],
                val.Data4[3], val.Data4[4], val.Data4[5], val.Data4[6], val.Data4[7]))
        },
        EvtVarTypeSid => {
            let sid = unsafe { variant.u.SidVal() };
            let mut string_sid : *mut u16 = null_mut();
            let slice : &[u16];
            let res = unsafe { ConvertSidToStringSidW(*sid as *mut c_void, &mut string_sid) };
            if res == 0 {
                return Err(format!("ConvertSidToStringSid() failed with code {}", get_win32_errcode()));
            }
            unsafe {
                let len = (0..).take_while(|&i| *string_sid.offset(i) != 0).count();
                slice = std::slice::from_raw_parts(string_sid, len);
                LocalFree(string_sid as *mut c_void);
            }
            match String::from_utf16(slice) {
                Ok(s) => EvtVariant::String(s),
                Err(e) => return Err(
                    format!("Cannot unwrap EVT_VARIANT: SID UTF16 conversion error: {}", e.to_string())),
            }
        }
        EvtVarTypeSizeT => {
            // SizeT is documented as a pointer type, but the value might have been generated
            // on another host than the runtime one (e.g. x64 event generator, x86 event collector)
            // so we use u64 in all cases, hoping that the structure was properly 0-initialized.
            let val : &usize = unsafe { variant.u.SizeTVal() };
            EvtVariant::UInt(*val as u64)
        },
        EvtVarTypeFileTime => {
            let val : FILETIME = unsafe { std::ptr::read(variant.u.FileTimeVal() as *const _ as *const FILETIME) };
            let mut stime: SYSTEMTIME = unsafe { std::mem::zeroed() };
            let convert_res = unsafe {
                FileTimeToSystemTime(&val, &mut stime)
            };
            if convert_res == 0 {
                return Err(format!("FileTimeToSystemTime() failed with code {}", get_win32_errcode()));
            }
            EvtVariant::DateTime(stime)
        },
        EvtVarTypeSysTime => {
            let stime : SYSTEMTIME = unsafe { std::ptr::read(variant.u.SysTimeVal() as *const _ as *const SYSTEMTIME) };
            EvtVariant::DateTime(stime)
        },
        EvtVarTypeHexInt64 => {
            let val : u64 = unsafe { std::ptr::read(&variant.u as *const _ as *const u64) };
            EvtVariant::UInt(val)
        },
        EvtVarTypeHexInt32 => {
            let val : u32 = unsafe { std::ptr::read(&variant.u as *const _ as *const u32) };
            EvtVariant::UInt(val as u64)
        },
        EvtVarTypeEvtHandle => {
            let val: EVT_HANDLE = unsafe { std::ptr::read(&variant.u as *const _ as *const EVT_HANDLE) };
            let handle = match EvtHandle::from_raw(val) {
                Ok(h) => h,
                Err(e) => return Err(format!("Unable to unwrap EvtVarTypeEvtHandle variant: {}", e)),
            };
            EvtVariant::Handle(handle)
        },
        unknown => {
            return Err(format!("Unsupported EVT_VARIANT type {} (count {}) (contents {})",
                               unknown, variant.Count, unsafe { std::ptr::read(&variant.u as *const _ as *const u64) }))
        },
    };

    // Microsoft created a whole typing system (see https://docs.microsoft.com/en-us/windows/win32/api/winevt/ne-winevt-evt_variant_type)
    // but somehow it got lost in the middle of the implementation... Event fields have types, but
    // the EvtQuery() API returns all fields as EvtVarTypeString... we cast what we can to repair it
    let res = match (res, type_hint) {
        (EvtVariant::String(s), Some("xs:string")) => EvtVariant::String(s),
        (EvtVariant::String(s), Some("xs:hexBinary")) => EvtVariant::String(s),
        (EvtVariant::String(s), Some("xs:GUID")) => EvtVariant::String(s),
        (EvtVariant::String(s), Some("win:HexInt64")) |
        (EvtVariant::String(s), Some("win:HexInt32")) |
        (EvtVariant::String(s), Some("win:HexInt16")) |
        (EvtVariant::String(s), Some("win:HexInt8")) =>
            if let Some(u) = hexstring_to_uint(&s) {
                EvtVariant::UInt(u)
            } else {
                EvtVariant::String(s)
            },
        (EvtVariant::String(s), Some(hint)) => {
            debug!("Please implement parsing from string to {} (e.g. {})", hint, s);
            EvtVariant::String(s)
        },
        (x, _) => x,
    };
    Ok(res)
}

fn get_event_common_property(buffer: *const u8, prop_num: u32) -> Result<EvtVariant, String> {
    let buffer_offset = (prop_num as usize) * std::mem::size_of::<EVT_VARIANT>();
    let prop : EVT_VARIANT = unsafe {
        std::ptr::read(buffer.add(buffer_offset) as *const _)
    };
    unwrap_variant_contents(&prop, None)
}

pub fn get_event_common_properties(h_event: &EvtHandle) -> Result<Option<CommonEventProperties>, String> {
    let h_ctxsystem = unsafe { EvtCreateRenderContext(0, null_mut(), EvtRenderContextSystem) };
    if h_ctxsystem.is_null() {
        return Err(format!("EvtCreateRenderContext(EvtRenderContextSystem) failed with code {}", get_win32_errcode()));
    }
    let h_ctxsystem = EvtHandle::from_raw( h_ctxsystem)?;

    let mut buffer_len_req : u32 = 0;
    let mut props_count : u32 = 0;
    let res = unsafe {
        EvtRender( h_ctxsystem.as_ptr(),
                  h_event.as_ptr(),
                  EvtRenderEventValues,
                  0,
                  null_mut(),
                  &mut buffer_len_req as *mut u32,
                  &mut props_count as *mut u32)
    };
    if res != 0 || get_win32_errcode() != ERROR_INSUFFICIENT_BUFFER {
        return Err(format!("EvtRender(EvtRenderEventValues) failed with code {}", get_win32_errcode()));
    }
    let mut buffer : Vec<u8> = Vec::with_capacity(buffer_len_req as usize);
    let res = unsafe {
        EvtRender( h_ctxsystem.as_ptr(),
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

    let timestamp = match get_event_common_property(buffer.as_ptr(), EvtSystemTimeCreated) {
        Ok(EvtVariant::DateTime(s)) => s,
        Ok(_) => return Err(format!("Unexpected EVT_VARIANT type for EvtSystemTimeCreated")),
        Err(e) => return Err(e),
    };
    let hostname = match get_event_common_property(buffer.as_ptr(), EvtSystemComputer) {
        Ok(EvtVariant::String(s)) => s,
        Ok(_) => return Err(format!("Unexpected EVT_VARIANT type for EvtSystemComputer")),
        Err(e) => return Err(e),
    };
    let recordid = match get_event_common_property(buffer.as_ptr(), EvtSystemEventRecordId) {
        Ok(EvtVariant::UInt(s)) => s,
        Ok(EvtVariant::Null) => {
            // Some events are just so called "bookmarks" inserted by a host so they can return
            // to their last position. No need to render those.
            // <Event xmlns='http://schemas.microsoft.com/win/2004/08/events/event'>
            //  <System>
            //      <Provider Name='Microsoft-Windows-EventForwarder'/>
            //      <EventID>111</EventID>
            //      <TimeCreated SystemTime='2019-10-19T17:07:45.416Z'/>
            //      <Computer>TEST</Computer>
            //  </System>
            //  <SubscriptionBookmarkEvent>
            //      <SubscriptionId></SubscriptionId>
            //  </SubscriptionBookmarkEvent>
            // </Event>
            return Ok(None);
        },
        Ok(_) => return Err(format!("Unexpected EVT_VARIANT type for EvtSystemEventRecordId")),
        Err(e) => return Err(e),
    };
    let provider = match get_event_common_property(buffer.as_ptr(), EvtSystemProviderName) {
        Ok(EvtVariant::String(s)) => s,
        Ok(_) => return Err(format!("Unexpected EVT_VARIANT type for EvtSystemProviderName")),
        Err(e) => return Err(e),
    };
    let eventid = match get_event_common_property(buffer.as_ptr(), EvtSystemEventID) {
        Ok(EvtVariant::UInt(s)) => s,
        Ok(_) => return Err(format!("Unexpected EVT_VARIANT type for EvtSystemEventID")),
        Err(e) => return Err(e),
    };
    let version = match get_event_common_property(buffer.as_ptr(), EvtSystemVersion) {
        Ok(EvtVariant::UInt(s)) => s,
        // Some events (e.g. Windows PowerShell/PowerShell/600) don't have versions...
        Ok(EvtVariant::Null) => 0,
        Ok(_) => return Err(format!("Unexpected EVT_VARIANT type for EvtSystemVersion")),
        Err(e) => return Err(e),
    };

    Ok(Some(CommonEventProperties {
        timestamp, hostname, recordid, provider, eventid, version
    }))
}