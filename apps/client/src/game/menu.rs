//! Screen stack, focused choices and input from game/menu.go.
use super::{
    MenuAction, Screen,
    wardrobe::{ROW_COUNT, cycle_row},
};
use crate::{
    character::Player,
    presentation::{Outfit, Wardrobe},
};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct Page {
    pub screen: Screen,
    pub focus: usize,
}
#[derive(Resource)]
pub struct Menu {
    pub stack: Vec<Page>,
    pub admitted: Option<f32>,
}
impl Default for Menu {
    fn default() -> Self {
        Self {
            admitted: None,
            stack: vec![Page {
                screen: Screen::Title,
                focus: 0,
            }],
        }
    }
}
impl Menu {
    pub fn screen(&self) -> Screen {
        self.stack.last().map_or(Screen::Playing, |p| p.screen)
    }
    pub fn focus(&self) -> usize {
        self.stack.last().map_or(0, |p| p.focus)
    }
    pub fn on_title(&self) -> bool {
        self.stack
            .first()
            .is_some_and(|p| p.screen == Screen::Title)
    }
    pub fn choices(&self) -> usize {
        choices(self.screen()).len()
            + if self.screen() == Screen::Dressing {
                ROW_COUNT
            } else {
                0
            }
    }
    pub fn set_focus(&mut self, focus: usize) {
        let n = self.choices();
        if n > 0
            && let Some(p) = self.stack.last_mut()
        {
            p.focus = focus % n;
        }
    }
    fn open(&mut self, screen: Screen) {
        self.stack.push(Page { screen, focus: 0 });
    }
    fn back(&mut self) {
        if !self.stack.is_empty() && !(self.stack.len() == 1 && self.on_title()) {
            self.stack.pop();
        }
    }
    pub fn apply(&mut self, action: MenuAction, wardrobe: &Wardrobe, outfit: &mut Outfit) {
        match action {
            MenuAction::Play if self.screen() == Screen::Title => {
                self.admitted = Some(0.0);
                self.stack.clear();
            }
            MenuAction::Pause if self.screen() == Screen::Playing => self.open(Screen::Paused),
            MenuAction::Resume if self.screen() == Screen::Paused => self.back(),
            MenuAction::Back => self.back(),
            MenuAction::MainMenu if self.screen() == Screen::Paused => *self = Self::default(),
            MenuAction::Wardrobe if matches!(self.screen(), Screen::Title | Screen::Paused) => {
                self.open(Screen::Dressing)
            }
            MenuAction::Controls if matches!(self.screen(), Screen::Title | Screen::Paused) => {
                self.open(Screen::Controls)
            }
            MenuAction::Focus(focus) => self.set_focus(focus),
            MenuAction::CycleRow { row, step }
                if self.screen() == Screen::Dressing && row < ROW_COUNT =>
            {
                self.set_focus(row);
                *outfit = cycle_row(wardrobe, *outfit, row, step);
            }
            _ => {}
        }
    }
    pub fn press(&self) -> Option<MenuAction> {
        if self.screen() == Screen::Dressing && self.focus() < ROW_COUNT {
            return Some(MenuAction::CycleRow {
                row: self.focus(),
                step: 1,
            });
        }
        let first = if self.screen() == Screen::Dressing {
            ROW_COUNT
        } else {
            0
        };
        choices(self.screen())
            .get(self.focus().saturating_sub(first))
            .map(|i| i.1)
    }
}
pub fn choices(screen: Screen) -> Vec<(&'static str, MenuAction)> {
    let mut items = match screen {
        Screen::Title => vec![
            ("Play", MenuAction::Play),
            ("Wardrobe", MenuAction::Wardrobe),
            ("Controls", MenuAction::Controls),
        ],
        Screen::Paused => vec![
            ("Resume", MenuAction::Resume),
            ("Wardrobe", MenuAction::Wardrobe),
            ("Controls", MenuAction::Controls),
            ("Main menu", MenuAction::MainMenu),
        ],
        Screen::Dressing => vec![("Done", MenuAction::Back)],
        Screen::Controls => vec![("Back", MenuAction::Back)],
        _ => vec![],
    };
    if !cfg!(target_arch = "wasm32") && matches!(screen, Screen::Title | Screen::Paused) {
        items.push(("Quit", MenuAction::Quit));
    }
    items
}
pub fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    screen: Res<State<Screen>>,
    mut menu: ResMut<Menu>,
    mut actions: MessageWriter<MenuAction>,
) {
    if *screen.get() == Screen::Loading {
        return;
    }
    if keys.any_just_pressed([KeyCode::Escape, KeyCode::Backspace]) {
        actions.write(if menu.screen() == Screen::Playing {
            MenuAction::Pause
        } else {
            MenuAction::Back
        });
        return;
    }
    let n = menu.choices();
    if n == 0 {
        return;
    }
    let focus = menu.focus();
    if keys.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW]) {
        menu.set_focus((focus + n - 1) % n);
    } else if keys.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS]) {
        menu.set_focus((focus + 1) % n);
    } else if menu.screen() == Screen::Dressing
        && focus < ROW_COUNT
        && keys.any_just_pressed([
            KeyCode::ArrowLeft,
            KeyCode::KeyA,
            KeyCode::ArrowRight,
            KeyCode::KeyD,
        ])
    {
        actions.write(MenuAction::CycleRow {
            row: focus,
            step: if keys.any_just_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]) {
                -1
            } else {
                1
            },
        });
    } else if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space])
        && let Some(action) = menu.press()
    {
        actions.write(action);
    }
}
pub fn actions(
    mut actions: MessageReader<MenuAction>,
    screen: Res<State<Screen>>,
    mut menu: ResMut<Menu>,
    mut next: ResMut<NextState<Screen>>,
    wardrobe: Res<Wardrobe>,
    mut players: Query<&mut Outfit, With<Player>>,
    mut exit: MessageWriter<AppExit>,
) {
    if *screen.get() == Screen::Loading {
        actions.clear();
        return;
    }
    let Ok(mut outfit) = players.single_mut() else {
        return;
    };
    for action in actions.read() {
        if matches!(action, MenuAction::Quit)
            && !cfg!(target_arch = "wasm32")
            && matches!(menu.screen(), Screen::Title | Screen::Paused)
        {
            exit.write(AppExit::Success);
        }
        menu.apply(*action, &wardrobe, &mut outfit);
    }
    if menu.screen() != *screen.get() {
        next.set(menu.screen());
    }
}
