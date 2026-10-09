//! Paperwork sounds for the currently playable menu and vehicle HUD.
use super::menu::Menu;
use super::{
    Screen,
    cues::{Cue, Voice},
};
use crate::{character::Player, presentation::Outfit, vehicle::Prompt};
use bevy::prelude::*;
#[derive(Resource, Default)]
pub struct UiSoundState {
    previous_map: Option<(bool, Vec2)>,
    previous: Option<(Screen, usize, usize, Option<Outfit>, String)>,
}
impl UiSoundState {
    pub fn navigation(&mut self, screen: Screen, marked: bool, dest: Vec2) -> Vec<Cue> {
        let mut out = vec![];
        if let Some((old_marked, old_dest)) = self.previous_map {
            if marked && (!old_marked || dest != old_dest) {
                out.push(Voice::ui("ui_mark", 0.8));
            } else if !marked && old_marked {
                out.push(if screen == Screen::Mapping {
                    Voice::ui("ui_mark", 0.6)
                } else {
                    Voice::ui("ui_arrive", 0.8)
                });
            }
        }
        self.previous_map = Some((marked, dest));
        out
    }
    pub fn update(
        &mut self,
        screen: Screen,
        depth: usize,
        focus: usize,
        outfit: Option<Outfit>,
        note: &str,
    ) -> Vec<Cue> {
        let mut out = vec![];
        if let Some((top, old_depth, old_focus, old_outfit, old_note)) = &self.previous {
            if *top != Screen::Loading {
                let sound = if depth > *old_depth && screen == Screen::Mapping {
                    Some(("ui_open", 0.8))
                } else if depth > *old_depth {
                    Some(("ui_page", 0.7))
                } else if depth < *old_depth && depth == 0 {
                    Some(("ui_stamp", 0.9))
                } else if depth < *old_depth {
                    Some(("ui_back", 0.6))
                } else if screen != *top {
                    Some(("ui_stamp", 0.9))
                } else if focus != *old_focus {
                    Some(("ui_move", 0.6))
                } else {
                    None
                };
                if let Some((name, volume)) = sound {
                    out.push(Voice::ui(name, volume));
                }
                if screen == Screen::Dressing && outfit != *old_outfit {
                    out.push(Voice::ui("cloth", 0.9));
                }
            }
            if !note.is_empty() && note != old_note {
                out.push(Voice::ui("ui_deny", 0.7));
            }
        }
        self.previous = Some((screen, depth, focus, outfit, note.into()));
        out
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn ui_cues(
    screen: Res<State<Screen>>,
    menu: Res<Menu>,
    players: Query<&Outfit, With<Player>>,
    prompt: Res<Prompt>,
    map: Option<Res<crate::landfall::maps::WorldMap>>,
    mut memory: ResMut<UiSoundState>,
    mut cues: MessageWriter<Cue>,
) {
    for cue in memory.update(
        if *screen.get() == Screen::Loading {
            Screen::Loading
        } else {
            menu.screen()
        },
        menu.stack.len(),
        menu.focus(),
        players.single().ok().copied(),
        prompt.noting(),
    ) {
        cues.write(cue);
    }
    let (marked, dest) = map
        .as_ref()
        .map_or((false, Vec2::ZERO), |m| (m.marked, m.dest));
    for cue in memory.navigation(menu.screen(), marked, dest) {
        cues.write(cue);
    }
}
