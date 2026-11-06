//! User-facing text in English and Brazilian Portuguese.
//!
//! Every [`Text`] must have a translation in each language; the exhaustive `match` in each
//! language function makes a missing translation a compile error.

/// A supported interface language.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Lang {
    #[default]
    En,
    PtBr,
}

impl Lang {
    /// Picks the first supported language from the user's preferred languages (BCP 47 tags,
    /// most preferred first), falling back to English.
    #[must_use]
    pub fn from_preferred<'a>(tags: impl IntoIterator<Item = &'a str>) -> Self {
        tags.into_iter()
            .find_map(|tag| {
                let primary = tag.split(['-', '_']).next()?.to_ascii_lowercase();
                match primary.as_str() {
                    "pt" => Some(Self::PtBr),
                    "en" => Some(Self::En),
                    _ => None,
                }
            })
            .unwrap_or_default()
    }
}

/// Identifier of a user-facing string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Text {
    IconAccessibility,
    IconTooltip,
    SeparatorAccessibility,
    AlwaysHiddenAccessibility,
    MenuRevealAll,
    MenuPreferences,
    MenuAbout,
    MenuQuit,
    MenuLaunchAtLogin,
    WarningSeparatorOrder,
    WarningShortcutUnavailable,
    LoginRequiresApproval,
    LoginUnavailable,
    SettingsTitle,
    SettingsGeneral,
    SettingsShortcut,
    SettingsShortcutRecord,
    SettingsShortcutRecording,
    SettingsShortcutClear,
    SettingsShortcutNone,
    SettingsAutoHide,
    SettingsAutoHideSeconds,
    SettingsAdvanced,
    SettingsAlwaysHidden,
    SettingsSecondaryBar,
    SettingsAccessibilityMissing,
    SettingsOpenSystemSettings,
}

/// Returns the translation of `text` in `lang`.
#[must_use]
pub fn text(lang: Lang, text: Text) -> &'static str {
    match lang {
        Lang::En => en(text),
        Lang::PtBr => pt_br(text),
    }
}

fn en(text: Text) -> &'static str {
    match text {
        Text::IconAccessibility => "Pinscher",
        Text::IconTooltip => "Click to show or hide icons. Right-click for options.",
        Text::SeparatorAccessibility => "Pinscher separator",
        Text::AlwaysHiddenAccessibility => "Pinscher always-hidden separator",
        Text::MenuRevealAll => "Reveal All Icons to Arrange…",
        Text::MenuPreferences => "Settings…",
        Text::MenuAbout => "About Pinscher",
        Text::MenuQuit => "Quit Pinscher",
        Text::MenuLaunchAtLogin => "Launch at Login",
        Text::WarningSeparatorOrder => {
            "Icons can't be hidden: the separator must be to the left of the Pinscher icon. Hold ⌘ and drag to reorder."
        }
        Text::WarningShortcutUnavailable => {
            "The keyboard shortcut is unavailable. Choose another one in Settings."
        }
        Text::LoginRequiresApproval => "Allow Pinscher in System Settings › General › Login Items.",
        Text::LoginUnavailable => {
            "Launch at login only works when Pinscher runs from its app bundle."
        }
        Text::SettingsTitle => "Pinscher Settings",
        Text::SettingsGeneral => "General",
        Text::SettingsShortcut => "Show or hide icons:",
        Text::SettingsShortcutRecord => "Record Shortcut",
        Text::SettingsShortcutRecording => "Press a shortcut… (Esc to cancel)",
        Text::SettingsShortcutClear => "Clear",
        Text::SettingsShortcutNone => "None",
        Text::SettingsAutoHide => "Hide icons again automatically after",
        Text::SettingsAutoHideSeconds => "seconds",
        Text::SettingsAdvanced => "Advanced",
        Text::SettingsAlwaysHidden => "Add an always-hidden section (second separator)",
        Text::SettingsSecondaryBar => {
            "Show hidden icons in a bar below the menu bar (⌥-click the icon)"
        }
        Text::SettingsAccessibilityMissing => {
            "Pinscher needs Accessibility permission for this feature."
        }
        Text::SettingsOpenSystemSettings => "Open System Settings",
    }
}

fn pt_br(text: Text) -> &'static str {
    match text {
        Text::IconAccessibility => "Pinscher",
        Text::IconTooltip => {
            "Clique para mostrar ou ocultar ícones. Clique com o botão direito para opções."
        }
        Text::SeparatorAccessibility => "Separador do Pinscher",
        Text::AlwaysHiddenAccessibility => "Separador sempre oculto do Pinscher",
        Text::MenuRevealAll => "Revelar todos os ícones para organizar…",
        Text::MenuPreferences => "Ajustes…",
        Text::MenuAbout => "Sobre o Pinscher",
        Text::MenuQuit => "Encerrar o Pinscher",
        Text::MenuLaunchAtLogin => "Iniciar com o login",
        Text::WarningSeparatorOrder => {
            "Não dá para ocultar ícones: o separador precisa ficar à esquerda do ícone do Pinscher. Segure ⌘ e arraste para reordenar."
        }
        Text::WarningShortcutUnavailable => {
            "O atalho de teclado não está disponível. Escolha outro nos Ajustes."
        }
        Text::LoginRequiresApproval => {
            "Permita o Pinscher em Ajustes do Sistema › Geral › Itens de Início."
        }
        Text::LoginUnavailable => {
            "Iniciar com o login só funciona quando o Pinscher roda a partir do app."
        }
        Text::SettingsTitle => "Ajustes do Pinscher",
        Text::SettingsGeneral => "Geral",
        Text::SettingsShortcut => "Mostrar ou ocultar ícones:",
        Text::SettingsShortcutRecord => "Gravar atalho",
        Text::SettingsShortcutRecording => "Pressione um atalho… (Esc cancela)",
        Text::SettingsShortcutClear => "Limpar",
        Text::SettingsShortcutNone => "Nenhum",
        Text::SettingsAutoHide => "Ocultar ícones de novo automaticamente após",
        Text::SettingsAutoHideSeconds => "segundos",
        Text::SettingsAdvanced => "Avançado",
        Text::SettingsAlwaysHidden => "Adicionar seção sempre oculta (segundo separador)",
        Text::SettingsSecondaryBar => {
            "Mostrar ícones ocultos numa barra abaixo da barra de menus (⌥ + clique no ícone)"
        }
        Text::SettingsAccessibilityMissing => {
            "O Pinscher precisa da permissão de Acessibilidade para este recurso."
        }
        Text::SettingsOpenSystemSettings => "Abrir Ajustes do Sistema",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_portuguese_for_any_pt_tag() {
        assert_eq!(Lang::from_preferred(["pt-BR"]), Lang::PtBr);
        assert_eq!(Lang::from_preferred(["pt_PT"]), Lang::PtBr);
        assert_eq!(Lang::from_preferred(["PT"]), Lang::PtBr);
    }

    #[test]
    fn picks_first_supported_language_in_order() {
        assert_eq!(Lang::from_preferred(["de-DE", "en-GB", "pt-BR"]), Lang::En);
        assert_eq!(Lang::from_preferred(["fr", "pt-BR", "en"]), Lang::PtBr);
    }

    #[test]
    fn falls_back_to_english() {
        assert_eq!(Lang::from_preferred(["ja-JP", "de"]), Lang::En);
        assert_eq!(Lang::from_preferred([]), Lang::En);
        assert_eq!(Lang::from_preferred([""]), Lang::En);
    }

    #[test]
    fn languages_differ_where_expected() {
        assert_ne!(
            text(Lang::En, Text::MenuQuit),
            text(Lang::PtBr, Text::MenuQuit)
        );
        assert_eq!(text(Lang::PtBr, Text::MenuQuit), "Encerrar o Pinscher");
    }
}
