//! The words a language catalogue owns: the launcher's routed chrome.
//!
//! A catalogue is original work, and it joins `Language::ALL` only when it
//! covers **every** `Key`. The per-catalogue `match` below is exhaustive by
//! the compiler, so a new `Key` cannot ship with a catalogue silently
//! missing it -- the build fails instead of the rail showing an empty
//! label. A tag no catalogue names falls back to the English default, which
//! is the same rule the reference's own loader follows for unknown locales.
//!
//! What is routed here is the chrome this file names: the navigation rail
//! and the settings tab strip. Page copy stays in the layout code, and this
//! launcher does not ship another product's translations.

use std::fmt;

/// The chrome words every catalogue must cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Home,
    Library,
    Discover,
    Settings,
    About,
    Appearance,
    Language,
    Java,
    Window,
    Resources,
    Privacy,
    Behaviour,
    Features,
}

/// A selectable language catalogue: the preference's stored tag and the
/// name the catalogue calls itself in its own language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Language {
    pub code: &'static str,
    pub name: &'static str,
}

impl Language {
    pub const ENGLISH: Self = Self {
        code: "en",
        name: "English",
    };
    /// Every catalogue here covers every [`Key`] -- enforced by the
    /// exhaustive matches in [`message`] and pinned by the tests below.
    pub const ALL: [Self; 4] = [
        Self::ENGLISH,
        Self {
            code: "de",
            name: "Deutsch",
        },
        Self {
            code: "fr",
            name: "Français",
        },
        Self {
            code: "ja",
            name: "日本語",
        },
    ];
    /// The catalogue a stored preference names, if any. An unknown tag
    /// selects nothing rather than lying about the active language.
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|language| language.code == code)
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

/// The routed word for `key` in `language`. An unknown tag reads the
/// English catalogue; a catalogue's own entries are complete, so a present
/// tag never falls through to another language word by word.
pub fn message(key: Key, language: &str) -> &'static str {
    match language {
        "de" => match key {
            Key::Home => "Start",
            Key::Library => "Sammlungen",
            Key::Discover => "Entdecken",
            Key::Settings => "Einstellungen",
            Key::About => "Über",
            Key::Appearance => "Darstellung",
            Key::Language => "Sprache",
            Key::Java => "Java & Spieler",
            Key::Window => "Fenster",
            Key::Resources => "Ressourcen",
            Key::Privacy => "Datenschutz",
            Key::Behaviour => "Verhalten",
            Key::Features => "Funktionen",
        },
        "fr" => match key {
            Key::Home => "Accueil",
            Key::Library => "Collections",
            Key::Discover => "Explorer",
            Key::Settings => "Préférences",
            Key::About => "À propos",
            Key::Appearance => "Apparence",
            Key::Language => "Langue",
            Key::Java => "Java et joueur",
            Key::Window => "Fenêtre",
            Key::Resources => "Ressources",
            Key::Privacy => "Confidentialité",
            Key::Behaviour => "Comportement",
            Key::Features => "Fonctions",
        },
        "ja" => match key {
            Key::Home => "ホーム",
            Key::Library => "コレクション",
            Key::Discover => "探す",
            Key::Settings => "設定",
            Key::About => "情報",
            Key::Appearance => "表示",
            Key::Language => "言語",
            Key::Java => "Java とプレイヤー",
            Key::Window => "ウィンドウ",
            Key::Resources => "リソース",
            Key::Privacy => "プライバシー",
            Key::Behaviour => "動作",
            Key::Features => "機能",
        },
        _ => match key {
            Key::Home => "Home",
            Key::Library => "Collections",
            Key::Discover => "Explore",
            Key::Settings => "Preferences",
            Key::About => "About",
            Key::Appearance => "Appearance",
            Key::Language => "Language",
            Key::Java => "Java & player",
            Key::Window => "Window",
            Key::Resources => "Resources",
            Key::Privacy => "Privacy",
            Key::Behaviour => "Behaviour",
            Key::Features => "Features",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every key a catalogue must cover. Production walks keys structurally
    /// -- the exhaustive matches in [`message`] -- so the enumeration
    /// belongs to the tests that pin coverage, not to the binary.
    const KEYS: [Key; 13] = [
        Key::Home,
        Key::Library,
        Key::Discover,
        Key::Settings,
        Key::About,
        Key::Appearance,
        Key::Language,
        Key::Java,
        Key::Window,
        Key::Resources,
        Key::Privacy,
        Key::Behaviour,
        Key::Features,
    ];

    #[test]
    fn every_catalogue_covers_every_key() {
        for language in Language::ALL {
            for key in KEYS {
                assert!(
                    !message(key, language.code).is_empty(),
                    "{} has no word for {key:?}",
                    language.code
                );
            }
        }
    }

    #[test]
    fn an_unnamed_tag_reads_the_english_catalogue() {
        for key in KEYS {
            assert_eq!(message(key, "en"), message(key, ""));
            assert_eq!(message(key, "en"), message(key, "xx"));
        }
    }

    #[test]
    fn catalogues_carry_their_own_words() {
        assert_eq!(message(Key::Home, "de"), "Start");
        assert_eq!(message(Key::Privacy, "de"), "Datenschutz");
        assert_eq!(message(Key::Privacy, "fr"), "Confidentialité");
        assert_eq!(message(Key::Features, "ja"), "機能");
        // English words that a translation legitimately shares are not
        // errors: the French catalogue spells collections the same way.
        assert_eq!(message(Key::Library, "fr"), "Collections");
    }

    #[test]
    fn languages_round_trip_through_their_tags() {
        for language in Language::ALL {
            assert_eq!(Language::from_code(language.code), Some(language));
        }
        assert_eq!(Language::from_code("xx"), None);
    }
}
