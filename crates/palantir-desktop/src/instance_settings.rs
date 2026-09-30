//! An instance's own settings, as a form: read back into controls and written
//! on save.
//!
//! Why this exists at all: [`crate::model`] inherited a settings *reader* from
//! `palantir-gui` -- the override-gate semantics Prism keeps (`OverrideMemory`
//! and its neighbours) -- and the note it left behind says the write side
//! belongs to the page that never arrived. This is that page, as a modal,
//! because a modal is where the reference keeps it: `InstanceSettingsModal`,
//! whose Java half is `components/settings-modal/java-settings.vue`.
//!
//! **Which half of the reference's modal this is.** That modal has tabs for
//! general, installation, sync overrides and sharing. What this module draws is
//! the settings an *instance file* holds and a launch reads -- the heap, the
//! Java path and the JVM arguments, each behind the same override switch the
//! reference draws a `Toggle` beside. The installation half (game version,
//! loader, loader version) is a different job -- its values come from a service
//! and writing it is an install, not a write -- and it is the open item next to
//! this one. The sync-override and sharing tabs are not built by decision
//! (G118: this launcher holds no Modrinth account to sync or share through).
//!
//! **What the form holds and what the file holds.** The controls are strings
//! while they are being typed ([`InstanceSettings`] is the parsed value),
//! because "2048x" is a keystroke rather than a heap and a form that refused a
//! keystroke would be a form nobody could type in. Save parses; a buffer that is
//! not a number is a sentence beside the fields rather than a written line, and
//! the heap's own floors are [`crate::store::Store::save_instance_settings`]'s
//! refusal because that is where the file is.
//!
//! Three readings are this module's rather than the reference's, and they are
//! marked where they are drawn: two memory fields where the reference has one
//! slider (a slider is a widget this kit does not have, and the pair is what the
//! instance file and Prism's own pane hold), their min/max labels, and the
//! Save button, which the reference does not need because its controls write on
//! every change.

use iced::widget::{column, row, text, text_input, Space};
use iced::{Alignment, Element, Length, Padding};

use crate::store::InstanceSettings;
use crate::style::{medium, semibold, INK_CONTRAST, INK_SECONDARY};
use crate::text_gen::Key;
use crate::theme_gen::{self, Theme as Gen};
use crate::ui;

/// What the form can be told.
#[derive(Debug, Clone)]
pub enum Message {
    /// The instance's own Java was switched on or off.
    OverrideJava(bool),
    /// The Java path field changed.
    JavaPath(String),
    /// The instance's own heap was switched on or off.
    OverrideMemory(bool),
    /// The heap floor field changed, in MiB.
    MemoryMin(String),
    /// The heap ceiling field changed, in MiB.
    MemoryMax(String),
    /// The instance's own JVM arguments were switched on or off.
    OverrideJavaArgs(bool),
    /// The JVM arguments field changed.
    JvmArgs(String),
    /// The pointer entered or left one of the form's controls, for the clock
    /// that carries a hover's 150 ms (see [`crate::ui`]).
    ///
    /// The form is not a page, but its Save button is drawn by [`ui::button`],
    /// which asks for a crossing like every other control in the kit -- so the
    /// form carries one, and [`State::update`] hands it to the clock.
    Hover {
        /// The control's stable name, one per control.
        key: &'static str,
        /// Whether the pointer arrived or left.
        over: bool,
        /// The hover end, where the control declares one of its own.
        hover: Option<f32>,
    },
    /// The form was asked to save.
    ///
    /// Handled by the shell rather than by [`State::update`], because saving is a
    /// write to a file the modal does not own: the shell is where the store is.
    Save,
}

crate::hovered!(Message);

/// The form: which instance, the three switches, and the four buffers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    /// Which instance, which is what a save writes to.
    pub id: String,
    /// Its name, for the dialog's title.
    pub name: String,
    /// Whether the form has values under it.
    ///
    /// False only for a read that failed: the modal is then the sentence alone,
    /// because a form of zeroes would be this launcher inventing settings for an
    /// instance it could not open.
    pub loaded: bool,
    /// Whether the instance's own Java is what a launch uses.
    pub override_java: bool,
    /// The Java path buffer.
    pub java_path: String,
    /// Whether the instance's own heap is what a launch uses.
    pub override_memory: bool,
    /// The heap floor buffer, in MiB.
    pub memory_min: String,
    /// The heap ceiling buffer, in MiB.
    pub memory_max: String,
    /// Whether the instance's own JVM arguments are what a launch uses.
    pub override_java_args: bool,
    /// The JVM arguments buffer.
    pub jvm_args: String,
    /// The last refusal, in the reader's words, or `None` while nothing failed.
    pub error: Option<String>,
}

impl State {
    /// A form filled from what the store read.
    ///
    /// The values are already the ones in force -- the store resolves an
    /// instance that overrides nothing to this launcher's own numbers -- so a
    /// switch drawn off still shows the reader what a launch would use.
    pub fn new(id: String, name: String, loaded: &InstanceSettings) -> State {
        State {
            id,
            name,
            loaded: true,
            override_java: loaded.override_java,
            java_path: loaded.java_path.clone(),
            override_memory: loaded.override_memory,
            memory_min: loaded.memory_min.to_string(),
            memory_max: loaded.memory_max.to_string(),
            override_java_args: loaded.override_java_args,
            jvm_args: loaded.jvm_args.clone(),
            error: None,
        }
    }

    /// The form for an instance that could not be read at all.
    ///
    /// The one state that draws no controls: the shell has a sentence and no
    /// values, and a form filled with defaults would be the launcher claiming an
    /// instance has a heap it never read.
    pub fn failed(id: String, name: String, problem: String) -> State {
        State {
            id,
            name,
            loaded: false,
            override_java: false,
            java_path: String::new(),
            override_memory: false,
            memory_min: String::new(),
            memory_max: String::new(),
            override_java_args: false,
            jvm_args: String::new(),
            error: Some(problem),
        }
    }

    /// The message applied.
    pub fn update(&mut self, message: Message) {
        match message {
            Message::OverrideJava(on) => self.override_java = on,
            Message::JavaPath(value) => self.java_path = value,
            Message::OverrideMemory(on) => self.override_memory = on,
            Message::MemoryMin(value) => self.memory_min = value,
            Message::MemoryMax(value) => self.memory_max = value,
            Message::OverrideJavaArgs(on) => self.override_java_args = on,
            Message::JvmArgs(value) => self.jvm_args = value,
            // The crossing is only the clock's: it is recorded here because
            // this is the form's `update`, and the button that reports it is
            // drawn on the next frame.
            Message::Hover { key, over, hover } => crate::ui::pointer_with(
                key,
                over,
                hover.unwrap_or_else(crate::theme::hover_brightness),
            ),
            // See `Message::Save`: the shell is the one that acts on it.
            Message::Save => {}
        }
    }

    /// The value a save would write, or the sentence that stops it.
    ///
    /// The number parse is here because this is where the buffers are; the
    /// store's own refusals -- a heap under the game's floor, or upside down --
    /// are its own, because that is where the file is.
    pub fn edit(&self) -> Result<InstanceSettings, String> {
        fn number(buffer: &str, what: &str) -> Result<i64, String> {
            buffer
                .trim()
                .parse::<i64>()
                .map_err(|_| format!("the {what} has to be a number of MiB; '{buffer}' is not one"))
        }
        let (memory_min, memory_max) = if self.override_memory {
            (
                number(&self.memory_min, "minimum heap")?,
                number(&self.memory_max, "maximum heap")?,
            )
        } else {
            // Nothing is written while the gate is off, so a buffer that no
            // longer parses cannot stand between the reader and their other two
            // settings.
            (0, 0)
        };
        Ok(InstanceSettings {
            java_path: self.java_path.clone(),
            override_java: self.override_java,
            memory_min,
            memory_max,
            override_memory: self.override_memory,
            jvm_args: self.jvm_args.clone(),
            override_java_args: self.override_java_args,
        })
    }
}

/// The form's body: three sections, each a switch over its own controls.
pub fn view(theme: Gen, state: &State) -> Element<'_, Message> {
    if !state.loaded {
        let sentence = state.error.clone().unwrap_or_default();
        return ui::admonition(theme, ui::Severity::Warning, "instance-settings", &sentence);
    }
    let mut body = column![]
        .spacing(12.0)
        .push(section(
            theme,
            state.override_java,
            Message::OverrideJava(!state.override_java),
            Key::InstanceSettingsTabsJavaCustomJavaInstallation,
            field(
                theme,
                Key::InstanceSettingsTabsJavaJavaPathPlaceholder.message(),
                &state.java_path,
                Message::JavaPath,
            ),
        ))
        .push(memory_section(theme, state))
        .push(section(
            theme,
            state.override_java_args,
            Message::OverrideJavaArgs(!state.override_java_args),
            Key::InstanceSettingsTabsJavaCustomJavaArguments,
            field(
                theme,
                Key::InstanceSettingsTabsJavaEnterJavaArguments.message(),
                &state.jvm_args,
                Message::JvmArgs,
            ),
        ));
    if let Some(error) = &state.error {
        body = body.push(ui::admonition(theme, ui::Severity::Warning, "instance-settings", error));
    }
    body.push(
        row![]
            .align_items(Alignment::Center)
            .push(Space::with_width(Length::Fill))
            .push(ui::button(
                theme,
                "instance-settings:save",
                Key::ButtonSave,
                ui::Kind::Colored,
                Message::Save,
            )),
    )
    .into()
}

/// The memory section: the switch, then the two numbers the instance file holds.
///
/// The reference's own memory control is a slider over the machine's RAM, which
/// this kit has none of; the pair of numbers is what the file and Prism's own
/// pane already hold, so the form edits the thing it will write. The two labels
/// are this module's words -- the reference has no label for them because it has
/// no fields -- and they say `MiB` for Prism's sake, whose keys are megabytes by
/// name and mebibytes by value.
fn memory_section(theme: Gen, state: &State) -> Element<'_, Message> {
    let number_field = |label: &'static str, value: &str, on_input: fn(String) -> Message| {
        column![]
            .spacing(4.0)
            .width(Length::Fill)
            .push(
                text(label.to_string())
                    .size(12.0)
                    .font(medium())
                    .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_SECONDARY))),
            )
            .push(field(theme, "1024", value, on_input))
    };
    section(
        theme,
        state.override_memory,
        Message::OverrideMemory(!state.override_memory),
        Key::InstanceSettingsTabsJavaCustomMemoryAllocation,
        row![]
            .spacing(8.0)
            .push(number_field("Minimum (MiB)", &state.memory_min, Message::MemoryMin))
            .push(number_field("Maximum (MiB)", &state.memory_max, Message::MemoryMax))
            .into(),
    )
}

/// One section: its override switch, its title, and the controls under it.
fn section<'a>(
    theme: Gen,
    on: bool,
    on_press: Message,
    title: Key,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    ui::card(
        theme,
        column![]
            .spacing(8.0)
            .push(
                row![]
                    .align_items(Alignment::Center)
                    .spacing(8.0)
                    .push(ui::switch(theme, on, on_press))
                    .push(
                        text(title.message())
                            .size(14.0)
                            .font(semibold())
                            .style(iced::theme::Text::Color(theme_gen::ink(theme, INK_CONTRAST))),
                    ),
            )
            .push(body),
    )
}

/// A text field on the dialog's own surface: the shell's create dialog field,
/// which is where this kit's bordered text style comes from.
fn field<'a>(
    theme: Gen,
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    text_input(placeholder, value)
        .on_input(on_input)
        .padding(Padding { top: 10.0, bottom: 10.0, left: 12.0, right: 12.0 })
        .size(14.0)
        .font(medium())
        .style(iced::theme::TextInput::Custom(Box::new(ui::Field::bordered(theme))))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loaded() -> InstanceSettings {
        InstanceSettings {
            java_path: "C:/jdk21/bin/javaw.exe".to_string(),
            override_java: true,
            memory_min: 2048,
            memory_max: 8192,
            override_memory: true,
            jvm_args: "-XX:+UseG1GC".to_string(),
            override_java_args: true,
        }
    }

    #[test]
    fn the_form_starts_where_the_file_is() {
        let state = State::new("atm10".to_string(), "All the Mods 10".to_string(), &loaded());
        assert_eq!(state.java_path, "C:/jdk21/bin/javaw.exe");
        assert_eq!(state.memory_min, "2048");
        assert_eq!(state.memory_max, "8192");
        assert_eq!(state.jvm_args, "-XX:+UseG1GC");
        assert!(state.override_memory && state.override_java && state.override_java_args);
        assert!(state.error.is_none());
        // A form that is opened and saved without a keystroke writes back what
        // it read: the round trip is the modal's whole promise.
        assert_eq!(state.edit().expect("an edit"), loaded());
    }

    #[test]
    fn a_field_that_is_not_a_number_is_a_sentence_rather_than_a_written_line() {
        let mut state = State::new("atm10".to_string(), "All the Mods 10".to_string(), &loaded());
        state.update(Message::MemoryMin("2048x".to_string()));
        let refused = state.edit().expect_err("not a number");
        assert!(refused.contains("minimum heap"), "{refused}");
        assert!(refused.contains("2048x"), "the sentence names what was typed: {refused}");

        // The other direction: a field that is off does not stand between the
        // reader and the settings that are on.
        state.update(Message::OverrideMemory(false));
        let edit = state.edit().expect("the gates that are on still save");
        assert!(!edit.override_memory);
        assert!(edit.override_java && edit.override_java_args);
    }

    #[test]
    fn the_switches_are_flipped_by_their_own_messages() {
        let mut state = State::new("atm10".to_string(), "All the Mods 10".to_string(), &loaded());
        state.update(Message::OverrideMemory(false));
        state.update(Message::OverrideJava(false));
        state.update(Message::OverrideJavaArgs(false));
        assert!(!state.override_memory && !state.override_java && !state.override_java_args);
        state.update(Message::OverrideJava(true));
        assert!(state.override_java && !state.override_memory);
        // Flipping a switch is not a save: the buffers stay where they were, so
        // a reader who changes their mind finds their numbers again.
        assert_eq!(state.memory_max, "8192");
    }
}
