//! Read-only Mission Control space discovery through SkyLight.
//!
//! This is intentionally a narrow Phase 5 boundary: it only discovers the
//! current space and ordered space ids for a display. Space mutation and event
//! subscriptions stay out of this module until the scripting-addition work.

#![cfg(target_os = "macos")]

use std::ffi::c_void;
use std::io;
use std::os::raw::c_char;

type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CFUUIDRef = *const c_void;
type CFArrayRef = *const c_void;
type CFDictionaryRef = *const c_void;
type CFNumberRef = *const c_void;
type CFAllocatorRef = *const c_void;
type CFIndex = isize;
type Boolean = u8;

// `kCFNumberSInt64Type` from <CoreFoundation/CFNumber.h>.
const K_CF_NUMBER_SINT64_TYPE: i32 = 4;
// `kCFStringEncodingUTF8`.
const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;

struct OwnedCf(CFTypeRef);

impl OwnedCf {
    fn as_ptr(&self) -> CFTypeRef {
        self.0
    }
}

impl Drop for OwnedCf {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: `OwnedCf` only wraps objects returned by Create/Copy
            // CoreFoundation APIs, so this balances one owned retain count.
            unsafe { CFRelease(self.0) };
        }
    }
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGDisplayCreateUUIDFromDisplayID(display: u32) -> CFUUIDRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFArrayGetCount(the_array: CFArrayRef) -> CFIndex;
    fn CFArrayGetValueAtIndex(the_array: CFArrayRef, idx: CFIndex) -> CFTypeRef;
    fn CFDictionaryGetValue(dict: CFDictionaryRef, key: *const c_void) -> *const c_void;
    fn CFEqual(cf1: CFTypeRef, cf2: CFTypeRef) -> Boolean;
    fn CFNumberGetValue(number: CFNumberRef, the_type: i32, value_ptr: *mut c_void) -> Boolean;
    fn CFRelease(cf: CFTypeRef);
    fn CFStringCreateWithCString(
        alloc: CFAllocatorRef,
        c_str: *const c_char,
        encoding: u32,
    ) -> CFStringRef;
    fn CFUUIDCreateString(alloc: CFAllocatorRef, uuid: CFUUIDRef) -> CFStringRef;
}

#[link(name = "SkyLight", kind = "framework")]
unsafe extern "C" {
    fn SLSMainConnectionID() -> i32;
    fn SLSManagedDisplayGetCurrentSpace(cid: i32, uuid: CFStringRef) -> u64;
    fn SLSCopyManagedDisplaySpaces(cid: i32) -> CFArrayRef;
}

fn owned_cfstring(literal: &[u8]) -> io::Result<OwnedCf> {
    debug_assert_eq!(literal.last(), Some(&0), "literal must be NUL-terminated");
    // SAFETY: `literal` is a valid NUL-terminated UTF-8 buffer and
    // CoreFoundation copies it into an owned CFString.
    let value = unsafe {
        CFStringCreateWithCString(
            std::ptr::null(),
            literal.as_ptr() as *const c_char,
            K_CF_STRING_ENCODING_UTF8,
        )
    };
    if value.is_null() {
        Err(io::Error::other("failed to create CoreFoundation string"))
    } else {
        Ok(OwnedCf(value))
    }
}

fn display_uuid_string(display_id: u32) -> io::Result<OwnedCf> {
    // SAFETY: `display_id` comes from CoreGraphics callers; the returned UUID,
    // if non-null, is owned and released after converting it to a CFString.
    let uuid = unsafe { CGDisplayCreateUUIDFromDisplayID(display_id) };
    if uuid.is_null() {
        return Err(io::Error::other(format!(
            "failed to create UUID for display {display_id}"
        )));
    }

    // SAFETY: `uuid` is a valid owned CFUUID; CoreFoundation returns an owned
    // string or null. The UUID is released immediately after conversion.
    let uuid_string = unsafe {
        let string = CFUUIDCreateString(std::ptr::null(), uuid);
        CFRelease(uuid);
        string
    };
    if uuid_string.is_null() {
        Err(io::Error::other(format!(
            "failed to stringify UUID for display {display_id}"
        )))
    } else {
        Ok(OwnedCf(uuid_string))
    }
}

fn cfnumber_u64(number: CFNumberRef) -> Option<u64> {
    if number.is_null() {
        return None;
    }

    let mut value = 0i64;
    // SAFETY: `number` is a CFNumber borrowed from a SkyLight dictionary and
    // `value` is a valid out pointer for the requested 64-bit integer type.
    let ok = unsafe {
        CFNumberGetValue(
            number,
            K_CF_NUMBER_SINT64_TYPE,
            &mut value as *mut i64 as *mut c_void,
        )
    };
    (ok != 0 && value > 0).then_some(value as u64)
}

/// Return Mission Control's current space id for `display_id`.
pub fn current_space_for_display(display_id: u32) -> io::Result<u64> {
    let uuid = display_uuid_string(display_id)?;
    // SAFETY: `SLSMainConnectionID` returns the process' SkyLight connection;
    // `uuid` is a valid CFString display identifier for the duration of the call.
    let sid = unsafe { SLSManagedDisplayGetCurrentSpace(SLSMainConnectionID(), uuid.as_ptr()) };
    if sid == 0 {
        Err(io::Error::other(format!(
            "failed to discover current space for display {display_id}"
        )))
    } else {
        Ok(sid)
    }
}

/// Return Mission Control's ordered space ids for `display_id`.
pub fn spaces_for_display(display_id: u32) -> io::Result<Vec<u64>> {
    let uuid = display_uuid_string(display_id)?;
    let display_identifier_key = owned_cfstring(b"Display Identifier\0")?;
    let spaces_key = owned_cfstring(b"Spaces\0")?;
    let id_key = owned_cfstring(b"id64\0")?;

    // SAFETY: `SLSMainConnectionID` returns the process' SkyLight connection;
    // the returned array is owned and released at the end of this function.
    let display_spaces = unsafe { SLSCopyManagedDisplaySpaces(SLSMainConnectionID()) };
    if display_spaces.is_null() {
        return Err(io::Error::other("failed to copy managed display spaces"));
    }
    let display_spaces = OwnedCf(display_spaces);

    let mut result = Vec::new();
    // SAFETY: `display_spaces` is a valid CFArray of display dictionaries; all
    // dictionary and nested-array values are borrowed and null-checked before use.
    unsafe {
        let display_count = CFArrayGetCount(display_spaces.as_ptr() as CFArrayRef);
        for display_index in 0..display_count {
            let display_ref =
                CFArrayGetValueAtIndex(display_spaces.as_ptr() as CFArrayRef, display_index)
                    as CFDictionaryRef;
            if display_ref.is_null() {
                continue;
            }

            let identifier = CFDictionaryGetValue(display_ref, display_identifier_key.as_ptr());
            if identifier.is_null() || CFEqual(uuid.as_ptr(), identifier) == 0 {
                continue;
            }

            let spaces_ref = CFDictionaryGetValue(display_ref, spaces_key.as_ptr()) as CFArrayRef;
            if spaces_ref.is_null() {
                break;
            }

            let space_count = CFArrayGetCount(spaces_ref);
            for space_index in 0..space_count {
                let space_ref = CFArrayGetValueAtIndex(spaces_ref, space_index) as CFDictionaryRef;
                if space_ref.is_null() {
                    continue;
                }
                let sid_ref = CFDictionaryGetValue(space_ref, id_key.as_ptr()) as CFNumberRef;
                if let Some(sid) = cfnumber_u64(sid_ref) {
                    result.push(sid);
                }
            }
            break;
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use crate::active_displays;

    use super::*;

    #[test]
    fn current_space_belongs_to_display_space_list() {
        let displays = active_displays().unwrap();
        let Some(display) = displays.first() else {
            return;
        };

        let current_sid = current_space_for_display(display.id).unwrap();
        let spaces = spaces_for_display(display.id).unwrap();
        assert!(spaces.contains(&current_sid));
    }
}
