//! Reads and presses other apps' menu bar items through Accessibility.
//!
//! AX objects stay in this adapter; callers receive only core items and system error codes.

use crate::core::layout::{RawItem, Rect};
use crate::platform::permissions;
use objc2_app_kit::NSWorkspace;
use objc2_application_services::{AXError, AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{CFArray, CFRetained, CFString, CFType, CGPoint, CGSize};
use std::ptr::NonNull;

/// The Accessibility error code returned by a press operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressError {
    pub code: AXError,
}

/// Status items of other running apps. Never requests permission.
#[must_use]
pub fn items() -> Vec<RawItem> {
    if !permissions::granted().accessibility {
        return Vec::new();
    }
    let own_pid = i32::try_from(std::process::id()).unwrap_or_default();
    let mut items = Vec::new();
    for app in NSWorkspace::sharedWorkspace().runningApplications() {
        let owner_pid = app.processIdentifier();
        if owner_pid == own_pid {
            continue;
        }
        let Ok(children) = children(owner_pid) else {
            continue;
        };
        let owner = app
            .bundleIdentifier()
            .or_else(|| app.localizedName())
            .map(|name| name.to_string())
            .unwrap_or_default();
        for child in children {
            let Some(frame) = frame(&child) else {
                continue;
            };
            items.push(RawItem {
                owner_pid,
                owner: owner.clone(),
                title: title(&child),
                frame,
            });
        }
    }
    items
}

/// Finds the same title within four points of `x` and presses the closest item.
///
/// # Errors
/// Returns an AX error code if access is denied, the item disappeared or pressing failed.
pub fn press(owner_pid: i32, title: &str, x: f64) -> Result<(), PressError> {
    if !permissions::granted().accessibility {
        return Err(PressError {
            code: AXError::APIDisabled,
        });
    }
    if owner_pid <= 0 || !x.is_finite() {
        return Err(PressError {
            code: AXError::IllegalArgument,
        });
    }
    let children = children(owner_pid).map_err(|code| PressError { code })?;
    let child = children
        .into_iter()
        .filter_map(|child| {
            let distance = (frame(&child)?.x - x).abs();
            (distance <= 4.0 && self::title(&child) == title).then_some((child, distance))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(child, _)| child)
        .ok_or(PressError {
            code: AXError::NoValue,
        })?;
    // SAFETY: `child` is a retained AXUIElement and AXPress is a standard AX action.
    let code = unsafe { child.perform_action(&CFString::from_str("AXPress")) };
    if code == AXError::Success {
        Ok(())
    } else {
        Err(PressError { code })
    }
}

fn children(pid: i32) -> Result<Vec<CFRetained<AXUIElement>>, AXError> {
    // SAFETY: the pid comes from NSRunningApplication or is validated by `press`.
    let app = unsafe { AXUIElement::new_application(pid) };
    timeout(&app)?;
    let extras = attribute(&app, "AXExtrasMenuBar")?
        .downcast::<AXUIElement>()
        .map_err(|_| AXError::Failure)?;
    timeout(&extras)?;
    let children = attribute(&extras, "AXChildren")?
        .downcast::<CFArray>()
        .map_err(|_| AXError::Failure)?;
    // SAFETY: Accessibility attribute arrays contain CFTypes. Each child's AX type is checked
    // below before use; retaining them keeps them alive after the array is released.
    let children = unsafe { children.cast_unchecked::<CFType>() };
    Ok(children
        .iter()
        .filter_map(|child| {
            let child = child.downcast::<AXUIElement>().ok()?;
            timeout(&child).ok()?;
            Some(child)
        })
        .collect())
}

fn timeout(element: &AXUIElement) -> Result<(), AXError> {
    // SAFETY: `element` is a valid AXUIElement and 0.25 is a positive timeout in seconds.
    let code = unsafe { element.set_messaging_timeout(0.25) };
    if code == AXError::Success {
        Ok(())
    } else {
        Err(code)
    }
}

fn attribute(element: &AXUIElement, name: &str) -> Result<CFRetained<CFType>, AXError> {
    let mut value = std::ptr::null();
    // SAFETY: `value` is a writable out pointer, and both inputs live through this call.
    let code = unsafe {
        element.copy_attribute_value(&CFString::from_str(name), NonNull::from(&mut value))
    };
    if code != AXError::Success {
        return Err(code);
    }
    let value = NonNull::new(value.cast_mut()).ok_or(AXError::NoValue)?;
    // SAFETY: the successful AX Copy function returned an owned (+1) CF object.
    Ok(unsafe { CFRetained::from_raw(value) })
}

fn title(element: &AXUIElement) -> String {
    ["AXTitle", "AXDescription", "AXIdentifier"]
        .into_iter()
        .find_map(|name| {
            let value = attribute(element, name).ok()?.downcast::<CFString>().ok()?;
            let value = value.to_string();
            (!value.is_empty()).then_some(value)
        })
        .unwrap_or_default()
}

fn frame(element: &AXUIElement) -> Option<Rect> {
    let position = attribute(element, "AXPosition")
        .ok()?
        .downcast::<AXValue>()
        .ok()?;
    let size = attribute(element, "AXSize")
        .ok()?
        .downcast::<AXValue>()
        .ok()?;
    let mut point = CGPoint::default();
    let mut dimensions = CGSize::default();
    // SAFETY: AXValue checks the requested type. Each out pointer has the correct size and
    // alignment for its requested CGPoint/CGSize and lives through the call.
    let valid = unsafe {
        position.value(AXValueType::CGPoint, NonNull::from(&mut point).cast())
            && size.value(AXValueType::CGSize, NonNull::from(&mut dimensions).cast())
    };
    (valid
        && [point.x, point.y, dimensions.width, dimensions.height]
            .iter()
            .all(|n| n.is_finite()))
    .then_some(Rect {
        x: point.x,
        y: point.y,
        width: dimensions.width,
        height: dimensions.height,
    })
}
