//! The panel's getting-started checklist: three steps, and the two rules around
//! them.
//!
//! `onboarding-checklist/index.vue` draws the section and
//! `providers/onboarding-checklist.ts` is where its three booleans come from.
//! They arrive over `plugin:onboarding-checklist|get_onboarding_checklist` -- a
//! Tauri plugin whose Rust is not vendored beside this tree -- so what can be read
//! here is the *shape* of that answer and the rules the frontend applies to it,
//! and both are written down in this file rather than guessed at in the shell:
//!
//! * The answer is four flags: `has_created_instance`, `has_logged_into_minecraft`,
//!   `has_logged_into_modrinth` and `show_checklist`. The generated
//!   `OnboardingChecklist` type beside the component is the whole of it.
//! * The section is drawn `v-if="isReady && showChecklist"`, and the friends list
//!   where the reader's friends would be is `v-show="showFriendsList"`, which
//!   `App.vue` defines as `!showChecklist || hasLoggedIntoModrinth`. The two are
//!   one decision read from two sides, which is why both are here.
//!
//! **Two of the three facts are local and the third is not.** Whether an instance
//! exists and whether an account is signed in are this launcher's own answers.
//! `has_logged_into_modrinth` is Modrinth's, and this launcher has no Modrinth
//! sign-in at all: it is `false`, and the step is drawn as the outstanding thing
//! rather than quietly ticked, because a launcher that cannot sign in has not
//! signed in.
//!
//! **`show_checklist` is the one rule this tree cannot measure.** The flag is the
//! plugin's, and the vendored frontend only *reads* it: nothing in it dismisses the
//! checklist, and the provider's own fold is an `&&` over the events, so the
//! frontend never decides it either. What this launcher reads instead is the two
//! facts it can finish a step with -- the checklist is up while an instance is
//! missing or no account is signed in -- see [`Checklist::show`], which says why
//! the third step is deliberately left out of that reading rather than pinning the
//! section forever. `GATES.md` G102 records the difference.

use crate::text_gen::Key;

/// One of the checklist's steps, in the order the reference stacks them.
///
/// `onboarding-checklist/index.vue`'s own `steps` array: make somewhere to play,
/// sign in to play it, sign in to Modrinth for the rest. The order matters
/// because it is the order the reference asks in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    /// *Create first instance*, which opens the creation flow.
    CreateInstance,
    /// *Sign in to Minecraft*, which opens the accounts card's sign-in.
    LoginMinecraft,
    /// *Sign in to Modrinth*, which this launcher has no flow for at all.
    LoginModrinth,
}

impl Step {
    /// All three, in the order the reference draws them.
    pub const ALL: [Step; 3] = [Step::CreateInstance, Step::LoginMinecraft, Step::LoginModrinth];

    /// The step's own copy, out of the generated table.
    ///
    /// The ids are the component's own (`onboarding-checklist.create-instance` and
    /// its two siblings), so the words are the reference's rather than a
    /// paraphrase of them.
    pub fn label(self) -> Key {
        match self {
            Step::CreateInstance => Key::OnboardingChecklistCreateInstance,
            Step::LoginMinecraft => Key::OnboardingChecklistLoginMinecraft,
            Step::LoginModrinth => Key::OnboardingChecklistLoginModrinth,
        }
    }
}

/// The three facts the checklist is drawn from.
///
/// A struct rather than three booleans passed around, because the two rules below
/// read all three and a caller that could pass them in the wrong order would draw
/// the wrong rows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Checklist {
    /// `has_created_instance`: the launcher's own instance list is not empty.
    created_instance: bool,
    /// `has_logged_into_minecraft`: an account is signed in.
    logged_into_minecraft: bool,
    /// `has_logged_into_modrinth`, which only Modrinth's own sign-in can make
    /// true and this launcher does not have one.
    logged_into_modrinth: bool,
}

impl Checklist {
    /// The checklist as those three facts have it.
    pub fn of(
        created_instance: bool,
        logged_into_minecraft: bool,
        logged_into_modrinth: bool,
    ) -> Checklist {
        Checklist { created_instance, logged_into_minecraft, logged_into_modrinth }
    }

    /// Whether the panel draws the section: `isReady && showChecklist`, read as
    /// "a step this launcher can *finish* is outstanding".
    ///
    /// The flag is the plugin's and cannot be read from this tree (see the module
    /// docs), so this is an inference rather than a measurement -- and the third
    /// step is deliberately not part of it. That step can never be finished here,
    /// so a section pinned on it would never go away, and what it carries is not
    /// lost by leaving it out: the prompt moves to the friends section beside it,
    /// which is where the reference draws the same sentence for a reader with no
    /// Modrinth session ([`Checklist::friends_visible`]). While the section *is*
    /// up the third step is drawn with the other two, unfinished and saying so,
    /// rather than being quietly dropped from it.
    pub fn show(&self) -> bool {
        !(self.created_instance && self.logged_into_minecraft)
    }

    /// Whether the friends list beside it is drawn: `App.vue`'s own
    /// `!showChecklist || hasLoggedIntoModrinth`.
    ///
    /// It reads oddly, and it is written down because it is what the reference
    /// does: the friends list is hidden *while the checklist is up*, and comes back
    /// as soon as the reader is signed into Modrinth. So a reader who has never
    /// signed in sees the getting-started steps where their friends would be,
    /// rather than an empty box where a service's answer goes.
    pub fn friends_visible(&self) -> bool {
        !self.show() || self.logged_into_modrinth
    }

    /// Whether `step` is done, which is what the row's own picture is drawn from.
    pub fn complete(&self, step: Step) -> bool {
        match step {
            Step::CreateInstance => self.created_instance,
            Step::LoginMinecraft => self.logged_into_minecraft,
            Step::LoginModrinth => self.logged_into_modrinth,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A checklist with every step done, which is the one state the section is not
    /// drawn in.
    fn all_done() -> Checklist {
        Checklist::of(true, true, true)
    }

    #[test]
    fn the_three_steps_are_the_reference_s_own_copy() {
        // Read out of `app-frontend/src/locales/en-US/index.json`, which is where
        // the generated table's words come from: four ids, so a paraphrase here
        // would be a difference from the reference rather than a translation of it.
        assert_eq!(Step::ALL.len(), 3);
        assert_eq!(Step::CreateInstance.label().message(), "Create first instance");
        assert_eq!(Step::LoginMinecraft.label().message(), "Sign in to Minecraft");
        assert_eq!(Step::LoginModrinth.label().message(), "Sign in to Modrinth");
        assert_eq!(Key::OnboardingChecklistTitle.message(), "Getting started");
        // And the order is the component's own array: a place to play, then the two
        // sign-ins.
        assert_eq!(
            Step::ALL,
            [Step::CreateInstance, Step::LoginMinecraft, Step::LoginModrinth]
        );
    }

    #[test]
    fn the_section_is_up_while_a_step_the_launcher_can_finish_is_outstanding() {
        // Each of the two local facts on its own keeps the section up, whatever
        // Modrinth's is -- that step is the one nobody can finish here.
        for (index, step) in Step::ALL[..2].iter().enumerate() {
            let mut facts = [true; 3];
            facts[index] = false;
            let checklist = Checklist::of(facts[0], facts[1], facts[2]);
            assert!(checklist.show(), "{step:?} is outstanding");
            assert!(!checklist.complete(*step));
            for other in Step::ALL.iter().filter(|other| other != &step) {
                assert!(checklist.complete(*other), "{other:?} is done");
            }
        }
        // And a launcher that has done nothing at all.
        let fresh = Checklist::of(false, false, false);
        assert!(fresh.show());
        assert!(Step::ALL.iter().all(|step| !fresh.complete(*step)));
        // The third step on its own does *not*: a step this launcher can never
        // finish does not hold a getting-started section up forever, and the
        // prompt it carries is drawn in the friends section instead.
        let local_done = Checklist::of(true, true, false);
        assert!(!local_done.show());
        assert!(!local_done.complete(Step::LoginModrinth), "and it is still outstanding");
    }

    #[test]
    fn the_friends_list_is_the_other_side_of_the_same_rule() {
        // `App.vue`: `showFriendsList = !showChecklist || hasLoggedIntoModrinth`.
        let fresh = Checklist::of(false, false, false);
        assert!(
            fresh.show() && !fresh.friends_visible(),
            "the steps stand where the friends list would be"
        );
        // Signed in to Modrinth with the checklist still up: the friends list is
        // drawn *beside* it, which is the case the rule exists for.
        assert!(Checklist::of(true, false, true).friends_visible());
        // Nothing outstanding, and the third fact either way.
        assert!(all_done().friends_visible());
        assert!(Checklist::of(true, true, false).friends_visible());
    }

    #[test]
    fn a_step_this_launcher_can_never_finish_moves_the_prompt_to_the_friends_section() {
        // The third fact is always false here. Once the two local steps are done
        // the section goes away and the friends list -- which the reference draws
        // for a reader with no Modrinth session as the same sign-in sentence --
        // takes its place: the prompt is moved rather than dropped.
        let local = Checklist::of(true, true, false);
        assert!(!local.show());
        assert!(local.friends_visible());
        // Until then it waits behind the steps, whatever Modrinth's fact is.
        assert!(!Checklist::of(false, false, false).friends_visible());
        assert!(!Checklist::of(true, false, false).friends_visible());
    }
}
