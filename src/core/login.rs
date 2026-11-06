//! Pure decisions for the launch-at-login menu item.

use crate::core::l10n::Text;

/// Current status reported by the operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginState {
    Enabled,
    Disabled,
    RequiresApproval,
    Unavailable,
}

/// Action requested when the launch-at-login item is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginRequest {
    Enable,
    Disable,
    OpenSettings,
}

/// Presentation and action for the launch-at-login menu item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginMenuItem {
    pub checked: bool,
    pub note: Option<Text>,
    pub action: Option<LoginRequest>,
}

/// Decides how the menu should present the current launch-at-login status.
#[must_use]
pub fn menu_item(state: LoginState) -> LoginMenuItem {
    match state {
        LoginState::Enabled => LoginMenuItem {
            checked: true,
            note: None,
            action: Some(LoginRequest::Disable),
        },
        LoginState::Disabled => LoginMenuItem {
            checked: false,
            note: None,
            action: Some(LoginRequest::Enable),
        },
        LoginState::RequiresApproval => LoginMenuItem {
            checked: true,
            note: Some(Text::LoginRequiresApproval),
            action: Some(LoginRequest::OpenSettings),
        },
        LoginState::Unavailable => LoginMenuItem {
            checked: false,
            note: Some(Text::LoginUnavailable),
            action: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_is_checked_and_requests_disable() {
        assert_eq!(
            menu_item(LoginState::Enabled),
            LoginMenuItem {
                checked: true,
                note: None,
                action: Some(LoginRequest::Disable),
            }
        );
    }

    #[test]
    fn disabled_is_unchecked_and_requests_enable() {
        assert_eq!(
            menu_item(LoginState::Disabled),
            LoginMenuItem {
                checked: false,
                note: None,
                action: Some(LoginRequest::Enable),
            }
        );
    }

    #[test]
    fn requires_approval_is_checked_and_opens_settings() {
        assert_eq!(
            menu_item(LoginState::RequiresApproval),
            LoginMenuItem {
                checked: true,
                note: Some(Text::LoginRequiresApproval),
                action: Some(LoginRequest::OpenSettings),
            }
        );
    }

    #[test]
    fn unavailable_is_disabled_with_explanation() {
        assert_eq!(
            menu_item(LoginState::Unavailable),
            LoginMenuItem {
                checked: false,
                note: Some(Text::LoginUnavailable),
                action: None,
            }
        );
    }
}
