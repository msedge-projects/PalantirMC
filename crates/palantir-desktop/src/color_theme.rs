//! The colour theme the user picked, and the list the reference offers.
//!
//! The enum lives here rather than in [`crate::theme`] because it is a *setting*,
//! not a palette: it survives the removal of the hand-written palette in the last
//! stage of the rewrite, and the list it offers is the reference's own.
//!
//! Every value in this module is quoted from `composables/use-theme.ts`:
//!
//! ```text
//! export const THEME_OPTIONS = ['dark', 'light', 'oled', 'retro', 'system'] as const
//! export const DARK_THEMES = ['dark', 'oled', 'retro'] as const
//! ```
//!
//! and so is the rule that decides which of them the appearance settings show,
//! from `components/ui/settings/display/AppearanceSettings.vue`:
//!
//! ```text
//! theme.options.filter(
//!     (option) => option !== 'retro' || appSettings.devMode || current.value.theme === 'retro',
//! )
//! ```
//!
//! Retro is a real theme the reference paints (`.retro-mode`, resolved by
//! `theme_gen`), and it is *hidden* rather than absent: it appears when developer
//! mode is on, and it stays visible for whoever has already selected it. That is
//! why this module has both a full list and a filter, and why neither is the
//! other: an option list that dropped retro would make the setting unreachable for
//! the users who have it, and one that always showed it would not be the
//! reference's interface.

use crate::text_gen::Key;

/// A colour theme, as offered in Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorTheme {
    /// The dark look, and the default.
    #[default]
    Dark,
    /// The light look.
    Light,
    /// Black surfaces, for OLED displays.
    Oled,
    /// The reference's retro look: green-on-grey terminal colours.
    Retro,
    /// Follow the operating system's app appearance.
    System,
}

impl ColorTheme {
    /// Every theme the reference knows, in `THEME_OPTIONS`' own order.
    ///
    /// The order is the reference's and is not sorted: dark and light are the two
    /// it offers first, then OLED, then retro, then the OS-following one. A test
    /// pins it, because this array is what a settings pane iterates.
    pub const ALL: [ColorTheme; 5] = [
        ColorTheme::Dark,
        ColorTheme::Light,
        ColorTheme::Oled,
        ColorTheme::Retro,
        ColorTheme::System,
    ];

    /// The themes the hand-written palette can paint.
    ///
    /// [`crate::theme`] has dark, light and OLED and no retro -- the reference's
    /// retro is in [`crate::theme_gen`]. The old appearance pane offers this list
    /// rather than [`ColorTheme::ALL`], so it cannot offer a card it would have to
    /// paint as something else; the new Settings modal, which paints from the
    /// generated table, offers [`ColorTheme::options`] instead.
    #[cfg(test)]
    pub const PAINTED: [ColorTheme; 4] = [
        ColorTheme::Dark,
        ColorTheme::Light,
        ColorTheme::Oled,
        ColorTheme::System,
    ];

    /// The options a settings pane shows, by the reference's own rule.
    ///
    /// `dev_mode` is the reference's `appSettings.devMode`, and `current` is the
    /// theme in force: retro is offered when either is true, which is exactly what
    /// the filter in `AppearanceSettings.vue` does.
    pub fn options(dev_mode: bool, current: ColorTheme) -> Vec<ColorTheme> {
        ColorTheme::ALL
            .into_iter()
            .filter(|option| *option != ColorTheme::Retro || dev_mode || current == ColorTheme::Retro)
            .collect()
    }

    /// Whether this is one of the reference's dark themes: `DARK_THEMES`.
    ///
    /// The reference uses it to remember which dark theme to return to when the
    /// user switches away from one, which is why it is a property of the setting
    /// rather than of a palette.
    ///
    /// `#[cfg(test)]` because nothing in the product asks it: the setting the
    /// shell reads is resolved through [`crate::shell::generated_theme`], and the
    /// palette's own notion of dark went with the shell that painted from it.
    /// What is left is the fixture whose test compares this list against
    /// `DARK_THEMES`.
    #[cfg(test)]
    pub const fn is_dark(self) -> bool {
        matches!(self, ColorTheme::Dark | ColorTheme::Oled | ColorTheme::Retro)
    }

    /// The generated key holding this theme's own label.
    ///
    /// From the reference's UI locale (`settings.display.theme.*`), so the label
    /// on a card is the reference's word for it rather than a second copy.
    pub const fn label_key(self) -> Key {
        match self {
            ColorTheme::Dark => Key::SettingsDisplayThemeDark,
            ColorTheme::Light => Key::SettingsDisplayThemeLight,
            ColorTheme::Oled => Key::SettingsDisplayThemeOled,
            ColorTheme::Retro => Key::SettingsDisplayThemeRetro,
            ColorTheme::System => Key::SettingsDisplayThemeSystem,
        }
    }

    /// The label on the card.
    #[cfg(test)]
    pub fn label(self) -> &'static str {
        self.label_key().message()
    }

    /// Stable id, for the settings file. Never localized and never reordered;
    /// renaming one silently resets the theme of anyone who picked it.
    pub const fn id(self) -> &'static str {
        match self {
            ColorTheme::Dark => "dark",
            ColorTheme::Light => "light",
            ColorTheme::Oled => "oled",
            ColorTheme::Retro => "retro",
            ColorTheme::System => "system",
        }
    }

    /// Parse a stored id. Anything unrecognized is the default rather than an
    /// error: a hand-edited or future settings file must still open.
    pub fn from_id(id: &str) -> ColorTheme {
        Self::ALL
            .into_iter()
            .find(|theme| theme.id() == id.trim().to_ascii_lowercase())
            .unwrap_or_default()
    }

    /// The concrete look this theme means.
    ///
    /// `System` is the only theme that depends on the machine, and it resolves to
    /// the ordinary dark look when the OS is dark -- an explicit OLED choice is the
    /// display's business, not the OS's, so "system dark" means dark and OLED stays
    /// something you ask for.
    pub const fn resolve(self, system_prefers_light: bool) -> ColorTheme {
        match self {
            ColorTheme::System if system_prefers_light => ColorTheme::Light,
            ColorTheme::System => ColorTheme::Dark,
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_options_are_the_reference_s_own_list_in_its_own_order() {
        // `THEME_OPTIONS`, verbatim: dark, light, oled, retro, system.
        assert_eq!(
            ColorTheme::ALL,
            [
                ColorTheme::Dark,
                ColorTheme::Light,
                ColorTheme::Oled,
                ColorTheme::Retro,
                ColorTheme::System,
            ]
        );
        assert_eq!(ColorTheme::ALL.len(), 5);
        assert_eq!(ColorTheme::default(), ColorTheme::Dark);
        // And the four the hand-written palette paints, retro left out.
        assert_eq!(ColorTheme::PAINTED.len(), 4);
        assert!(!ColorTheme::PAINTED.contains(&ColorTheme::Retro));
    }

    #[test]
    fn retro_is_hidden_unless_developer_mode_or_already_chosen() {
        // The reference's filter, all four cases: off and not chosen (hidden),
        // developer mode (shown), already chosen (kept), and both.
        let plain = ColorTheme::options(false, ColorTheme::Dark);
        assert_eq!(plain, ColorTheme::PAINTED.to_vec());
        assert!(!plain.contains(&ColorTheme::Retro));

        let developer = ColorTheme::options(true, ColorTheme::Dark);
        assert_eq!(developer.len(), 5);
        assert!(developer.contains(&ColorTheme::Retro));

        let chosen = ColorTheme::options(false, ColorTheme::Retro);
        assert!(
            chosen.contains(&ColorTheme::Retro),
            "a theme that is in force cannot be hidden from the pane that shows it"
        );
        assert_eq!(chosen.len(), 5);
        assert_eq!(ColorTheme::options(true, ColorTheme::Retro).len(), 5);
        // The order survives the filter, so a card does not move when retro
        // appears beside it.
        assert_eq!(
            developer,
            vec![
                ColorTheme::Dark,
                ColorTheme::Light,
                ColorTheme::Oled,
                ColorTheme::Retro,
                ColorTheme::System,
            ]
        );
    }

    #[test]
    fn the_theme_in_force_is_painted_and_system_follows_the_machine() {
        // `resolve` touches nothing but `System`, retro included: retro is dark,
        // but it is not "the dark look's name for a light machine".
        assert_eq!(ColorTheme::Dark.resolve(false), ColorTheme::Dark);
        assert_eq!(ColorTheme::Dark.resolve(true), ColorTheme::Dark);
        assert_eq!(ColorTheme::Retro.resolve(true), ColorTheme::Retro);
        assert_eq!(ColorTheme::Oled.resolve(true), ColorTheme::Oled);
        assert_eq!(ColorTheme::System.resolve(false), ColorTheme::Dark);
        assert_eq!(ColorTheme::System.resolve(true), ColorTheme::Light);
    }

    #[test]
    fn dark_themes_are_the_reference_s_own_set() {
        // `DARK_THEMES = ['dark', 'oled', 'retro']`.
        for theme in ColorTheme::ALL {
            let expected = matches!(
                theme,
                ColorTheme::Dark | ColorTheme::Oled | ColorTheme::Retro
            );
            assert_eq!(theme.is_dark(), expected, "{}", theme.id());
        }
    }

    #[test]
    fn a_theme_s_id_is_stable_and_its_label_comes_from_the_reference() {
        let mut ids: Vec<&str> = ColorTheme::ALL.iter().map(|theme| theme.id()).collect();
        assert_eq!(ids, ["dark", "light", "oled", "retro", "system"]);
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(count, ids.len(), "two themes share an id");
        for theme in ColorTheme::ALL {
            assert_eq!(ColorTheme::from_id(theme.id()), theme);
        }
        // The labels are the reference's own words, from its UI locale: the four
        // it shows without developer mode plus retro's.
        assert_eq!(ColorTheme::Dark.label(), "Dark");
        assert_eq!(ColorTheme::Light.label(), "Light");
        assert_eq!(ColorTheme::Oled.label(), "OLED");
        assert_eq!(ColorTheme::Retro.label(), "Retro");
        assert_eq!(ColorTheme::System.label(), "Sync with system");
        // Every label key is the reference's key, spelled out, rather than a
        // second table of strings kept beside the generated one.
        assert_eq!(ColorTheme::Retro.label_key().name(), "settings.display.theme.retro");
    }

    #[test]
    fn an_unknown_stored_id_opens_as_the_default() {
        assert_eq!(ColorTheme::from_id("dark"), ColorTheme::Dark);
        assert_eq!(ColorTheme::from_id(" DARK "), ColorTheme::Dark);
        assert_eq!(ColorTheme::from_id("retro"), ColorTheme::Retro);
        assert_eq!(ColorTheme::from_id(""), ColorTheme::Dark);
        assert_eq!(ColorTheme::from_id("solarized"), ColorTheme::Dark);
    }
}
