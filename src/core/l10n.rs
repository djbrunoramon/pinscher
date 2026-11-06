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
    MenuPreferences,
    MenuAbout,
    MenuQuit,
    MenuLaunchAtLogin,
    WarningSeparatorOrder,
    WarningShortcutUnavailable,
    LoginRequiresApproval,
    LoginUnavailable,
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
        Text::MenuPreferences => "Preferences…",
        Text::MenuAbout => "About Pinscher",
        Text::MenuQuit => "Quit Pinscher",
        Text::MenuLaunchAtLogin => "Launch at Login",
        Text::WarningSeparatorOrder => {
            "Icons can't be hidden: the separator must be to the left of the Pinscher icon. Hold ⌘ and drag to reorder."
        }
        Text::WarningShortcutUnavailable => {
            "The keyboard shortcut is unavailable. Choose another one in Preferences."
        }
        Text::LoginRequiresApproval => "Allow Pinscher in System Settings › General › Login Items.",
        Text::LoginUnavailable => {
            "Launch at login only works when Pinscher runs from its app bundle."
        }
    }
}

fn pt_br(text: Text) -> &'static str {
    match text {
        Text::IconAccessibility => "Pinscher",
        Text::IconTooltip => {
            "Clique para mostrar ou ocultar ícones. Clique com o botão direito para opções."
        }
        Text::SeparatorAccessibility => "Separador do Pinscher",
        Text::MenuPreferences => "Preferências…",
        Text::MenuAbout => "Sobre o Pinscher",
        Text::MenuQuit => "Encerrar o Pinscher",
        Text::MenuLaunchAtLogin => "Iniciar com o login",
        Text::WarningSeparatorOrder => {
            "Não dá para ocultar ícones: o separador precisa ficar à esquerda do ícone do Pinscher. Segure ⌘ e arraste para reordenar."
        }
        Text::WarningShortcutUnavailable => {
            "O atalho de teclado não está disponível. Escolha outro nas Preferências."
        }
        Text::LoginRequiresApproval => {
            "Permita o Pinscher em Ajustes do Sistema › Geral › Itens de Início."
        }
        Text::LoginUnavailable => {
            "Iniciar com o login só funciona quando o Pinscher roda a partir do app."
        }
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
