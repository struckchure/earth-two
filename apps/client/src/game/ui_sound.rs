//! Paperwork sounds for the currently playable menu and vehicle HUD.
use super::{
    Screen,
    cues::{Cue, Voice},
};
use crate::vehicle::Prompt;
use bevy::prelude::*;
/// Shared with the renderer so keyboard focus has one source of truth.
#[derive(Resource, Default)]
pub struct MenuFocus(pub usize);
#[derive(Resource, Default)]
pub struct UiSoundState {
    previous: Option<(Screen, usize, String)>,
}
impl UiSoundState {
    pub fn update(&mut self, screen: Screen, focus: usize, note: &str) -> Vec<Cue> {
        let mut out = vec![];
        if let Some((top, old_focus, old_note)) = &self.previous {
            if screen != *top {
                // Loading is not a Go menu layer. Silence its initial handoff.
                if *top != Screen::Loading {
                    let (name, volume) = match screen {
                        Screen::Playing | Screen::Title => ("ui_stamp", 0.9),
                        Screen::Paused => ("ui_page", 0.7),
                        Screen::Loading => ("ui_back", 0.6),
                    };
                    out.push(Voice::ui(name, volume));
                }
            } else if screen != Screen::Playing && focus != *old_focus {
                out.push(Voice::ui("ui_move", 0.6));
            }
            if !note.is_empty() && note != old_note {
                out.push(Voice::ui("ui_deny", 0.7));
            }
        }
        self.previous = Some((screen, focus, note.into()));
        out
    }
}
pub(super) fn ui_cues(
    screen: Res<State<Screen>>,
    focus: Option<Res<MenuFocus>>,
    prompt: Res<Prompt>,
    mut memory: ResMut<UiSoundState>,
    mut cues: MessageWriter<Cue>,
) {
    for cue in memory.update(
        *screen.get(),
        focus.as_ref().map_or(0, |f| f.0),
        prompt.noting(),
    ) {
        cues.write(cue);
    }
}
