use std::mem;
use std::os::raw::c_void;
use std::ptr;

type AudioObjectID = u32;
type OSStatus = i32;

#[repr(C)]
struct AudioObjectPropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

const SYSTEM_OBJECT: AudioObjectID = 1;
const GLOBAL: u32 = u32::from_be_bytes(*b"glob");
const OUTPUT: u32 = u32::from_be_bytes(*b"outp");
const MAIN: u32 = 0;
const DEFAULT_OUTPUT: u32 = u32::from_be_bytes(*b"dOut");
const VOLUME_SCALAR: u32 = u32::from_be_bytes(*b"volm");
const MUTE: u32 = u32::from_be_bytes(*b"mute");
const DEVICE_NAME: u32 = u32::from_be_bytes(*b"lnam");

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    fn AudioObjectGetPropertyData(
        object: AudioObjectID,
        address: *const AudioObjectPropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        data_size: *mut u32,
        data: *mut c_void,
    ) -> OSStatus;
}

#[derive(Clone, Debug, Default)]
pub struct AudioReading {
    pub device: Option<String>,
    pub volume: Option<f32>,
    pub muted: bool,
}

pub fn read_output() -> AudioReading {
    unsafe { read_output_inner() }.unwrap_or_default()
}

unsafe fn read_output_inner() -> Option<AudioReading> {
    let addr = AudioObjectPropertyAddress {
        selector: DEFAULT_OUTPUT,
        scope: GLOBAL,
        element: MAIN,
    };
    let mut device: AudioObjectID = 0;
    let mut size = mem::size_of::<AudioObjectID>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            SYSTEM_OBJECT,
            &addr,
            0,
            ptr::null(),
            &mut size,
            &mut device as *mut _ as *mut c_void,
        )
    };
    if status != 0 || device == 0 {
        return None;
    }

    let mut reading = AudioReading {
        device: unsafe { cf_string_prop(device, DEVICE_NAME, GLOBAL) },
        ..AudioReading::default()
    };

    let mut volumes = Vec::new();
    for element in [MAIN, 1, 2] {
        if let Some(v) = unsafe { f32_prop(device, VOLUME_SCALAR, OUTPUT, element) } {
            volumes.push(v);
        }
        if let Some(v) = unsafe { f32_prop(device, VOLUME_SCALAR, GLOBAL, element) } {
            volumes.push(v);
        }
    }
    if !volumes.is_empty() {
        reading.volume = Some(volumes.iter().copied().sum::<f32>() / volumes.len() as f32);
    }

    for element in [MAIN, 1, 2] {
        if let Some(m) = unsafe { u32_prop(device, MUTE, OUTPUT, element) } {
            reading.muted = m != 0;
            break;
        }
        if let Some(m) = unsafe { u32_prop(device, MUTE, GLOBAL, element) } {
            reading.muted = m != 0;
            break;
        }
    }
    Some(reading)
}

unsafe fn f32_prop(id: AudioObjectID, selector: u32, scope: u32, element: u32) -> Option<f32> {
    let addr = AudioObjectPropertyAddress {
        selector,
        scope,
        element,
    };
    let mut value: f32 = 0.0;
    let mut size = mem::size_of::<f32>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            id,
            &addr,
            0,
            ptr::null(),
            &mut size,
            &mut value as *mut _ as *mut c_void,
        )
    };
    (status == 0).then_some(value.clamp(0.0, 1.0))
}

unsafe fn u32_prop(id: AudioObjectID, selector: u32, scope: u32, element: u32) -> Option<u32> {
    let addr = AudioObjectPropertyAddress {
        selector,
        scope,
        element,
    };
    let mut value: u32 = 0;
    let mut size = mem::size_of::<u32>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            id,
            &addr,
            0,
            ptr::null(),
            &mut size,
            &mut value as *mut _ as *mut c_void,
        )
    };
    (status == 0).then_some(value)
}

unsafe fn cf_string_prop(id: AudioObjectID, selector: u32, scope: u32) -> Option<String> {
    let addr = AudioObjectPropertyAddress {
        selector,
        scope,
        element: MAIN,
    };
    let mut cfstr: *const c_void = ptr::null();
    let mut size = mem::size_of::<*const c_void>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            id,
            &addr,
            0,
            ptr::null(),
            &mut size,
            &mut cfstr as *mut _ as *mut c_void,
        )
    };
    if status != 0 || cfstr.is_null() {
        return None;
    }
    unsafe { cfstring_to_rust(cfstr) }
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringGetLength(s: *const c_void) -> isize;
    fn CFStringGetCString(
        s: *const c_void,
        buf: *mut i8,
        size: isize,
        encoding: u32,
    ) -> u8;
    fn CFRelease(cf: *const c_void);
}

unsafe fn cfstring_to_rust(s: *const c_void) -> Option<String> {
    let len = unsafe { CFStringGetLength(s) };
    if len <= 0 {
        unsafe { CFRelease(s) };
        return None;
    }
    let mut buf = vec![0i8; (len as usize) * 4 + 1];
    let ok = unsafe { CFStringGetCString(s, buf.as_mut_ptr(), buf.len() as isize, 0x0800_0100) };
    unsafe { CFRelease(s) };
    if ok == 0 {
        return None;
    }
    let bytes: Vec<u8> = buf.iter().map(|b| *b as u8).take_while(|b| *b != 0).collect();
    String::from_utf8(bytes).ok()
}
