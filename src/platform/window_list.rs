//! Reads status item windows from the window server.
//!
//! Without Screen Recording permission the window server omits titles (and, on recent macOS,
//! most windows), so callers must treat an empty result as "unknown", not "no items".

use crate::core::layout::{RawWindow, Rect};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSRunningApplication;
use objc2_core_foundation::{CFArray, CFString};
use objc2_core_graphics::{
    CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID, kCGWindowBounds,
    kCGWindowLayer, kCGWindowName, kCGWindowNumber, kCGWindowOwnerName, kCGWindowOwnerPID,
};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};
use std::collections::HashMap;

type WindowInfo = NSDictionary<NSString, AnyObject>;

/// Every window the window server reports, including off-screen ones.
#[must_use]
pub fn windows() -> Vec<RawWindow> {
    let Some(list) = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, kCGNullWindowID)
    else {
        return Vec::new();
    };
    // SAFETY: CFArray is toll-free bridged with NSArray, and the window list contains
    // CFDictionary (bridged with NSDictionary) values keyed by CFString (bridged with NSString).
    let list: &NSArray<WindowInfo> = unsafe { &*std::ptr::from_ref::<CFArray>(&list).cast() };

    let keys = Keys::get();
    let mut owners: HashMap<i32, String> = HashMap::new();
    list.iter()
        .filter_map(|info| {
            let owner_pid = int(&info, keys.owner_pid)?;
            let owner = owners
                .entry(owner_pid)
                .or_insert_with(|| owner_name(&info, keys.owner_name, owner_pid))
                .clone();
            Some(RawWindow {
                id: u32::try_from(int(&info, keys.number)?).ok()?,
                layer: int(&info, keys.layer)?,
                owner_pid,
                owner,
                title: string(&info, keys.name).unwrap_or_default(),
                frame: bounds(&info, keys.bounds)?,
            })
        })
        .collect()
}

/// Window info dictionary keys, bridged to `NSString`.
struct Keys {
    owner_pid: &'static NSString,
    owner_name: &'static NSString,
    number: &'static NSString,
    layer: &'static NSString,
    name: &'static NSString,
    bounds: &'static NSString,
}

impl Keys {
    fn get() -> Self {
        // SAFETY: these are immutable CFString constants exported by CoreGraphics, valid for
        // the life of the process.
        unsafe {
            Self {
                owner_pid: bridge(kCGWindowOwnerPID),
                owner_name: bridge(kCGWindowOwnerName),
                number: bridge(kCGWindowNumber),
                layer: bridge(kCGWindowLayer),
                name: bridge(kCGWindowName),
                bounds: bridge(kCGWindowBounds),
            }
        }
    }
}

fn bridge(cf: &'static CFString) -> &'static NSString {
    // SAFETY: CFString is toll-free bridged with NSString.
    unsafe { &*std::ptr::from_ref(cf).cast() }
}

fn int(info: &WindowInfo, key: &NSString) -> Option<i32> {
    let value = info.objectForKey(key)?;
    let number = value.downcast_ref::<NSNumber>()?;
    Some(number.intValue())
}

fn float(info: &NSDictionary<NSString, AnyObject>, key: &str) -> Option<f64> {
    let value = info.objectForKey(&NSString::from_str(key))?;
    Some(value.downcast_ref::<NSNumber>()?.doubleValue())
}

fn string(info: &WindowInfo, key: &NSString) -> Option<String> {
    let value = info.objectForKey(key)?;
    Some(value.downcast_ref::<NSString>()?.to_string())
}

fn bounds(info: &WindowInfo, key: &NSString) -> Option<Rect> {
    let value: Retained<AnyObject> = info.objectForKey(key)?;
    let rect = value.downcast_ref::<NSDictionary>()?;
    // SAFETY: the bounds dictionary is keyed by NSString with NSNumber values.
    let rect: &NSDictionary<NSString, AnyObject> = unsafe { &*std::ptr::from_ref(rect).cast() };
    Some(Rect {
        x: float(rect, "X")?,
        y: float(rect, "Y")?,
        width: float(rect, "Width")?,
        height: float(rect, "Height")?,
    })
}

/// Bundle identifier of the process, falling back to the window's owner name.
fn owner_name(info: &WindowInfo, key: &NSString, pid: i32) -> String {
    NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
        .and_then(|app| app.bundleIdentifier())
        .map(|id| id.to_string())
        .or_else(|| string(info, key))
        .unwrap_or_default()
}
