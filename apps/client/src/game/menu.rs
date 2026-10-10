//! Screen stack, focused choices and input from game/menu.go.
use super::{
    MenuAction, Screen,
    wardrobe::{ROW_COUNT, cycle_row},
};
use crate::{
    character::Player,
    identity::{IDENTITY_FIELDS, IdentityAction, IdentityPanel},
    presentation::{Outfit, Wardrobe},
};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct Page {
    pub screen: Screen,
    pub focus: usize,
    pub receipt: usize,
}
#[derive(Resource)]
pub struct Menu {
    pub stack: Vec<Page>,
    pub admitted: Option<f32>,
    pub contract_stamp: Option<super::contracts::ContractStamp>,
}
impl Default for Menu {
    fn default() -> Self {
        Self {
            admitted: None,
            contract_stamp: None,
            stack: vec![Page {
                screen: Screen::Title,
                focus: 0,
                receipt: 0,
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
            } else if self.screen() == Screen::Identity {
                IDENTITY_FIELDS
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
    pub(super) fn open(&mut self, screen: Screen) {
        self.stack.push(Page {
            screen,
            focus: 0,
            receipt: 0,
        });
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
            MenuAction::Identity if matches!(self.screen(), Screen::Title | Screen::Paused) => {
                self.open(Screen::Identity)
            }
            MenuAction::Settings if matches!(self.screen(), Screen::Title | Screen::Paused) => {
                self.open(Screen::Settings)
            }
            MenuAction::Journal if self.screen() == Screen::Playing => {
                self.open(Screen::ContractJournal)
            }
            MenuAction::AcceptContract if self.screen() == Screen::ContractOffer => {
                self.contract_stamp = Some(super::contracts::ContractStamp {
                    screen: Screen::ContractOffer,
                    age: 0.,
                });
                self.back();
            }
            MenuAction::Map if self.screen() == Screen::Playing => self.open(Screen::Mapping),
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
        if self.screen() == Screen::Identity && self.focus() < IDENTITY_FIELDS {
            return Some(MenuAction::Focus((self.focus() + 1) % IDENTITY_FIELDS));
        }
        let first = if self.screen() == Screen::Dressing {
            ROW_COUNT
        } else if self.screen() == Screen::Identity {
            IDENTITY_FIELDS
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
            ("Identity", MenuAction::Identity),
            ("Settings", MenuAction::Settings),
        ],
        Screen::Paused => vec![
            ("Resume", MenuAction::Resume),
            ("Wardrobe", MenuAction::Wardrobe),
            ("Controls", MenuAction::Controls),
            ("Main menu", MenuAction::MainMenu),
            ("Identity", MenuAction::Identity),
            ("Settings", MenuAction::Settings),
        ],
        Screen::Identity => IdentityAction::ITEMS
            .into_iter()
            .map(|(label, action)| {
                (
                    label,
                    if action == IdentityAction::Back {
                        MenuAction::Back
                    } else {
                        MenuAction::IdentityAct(action)
                    },
                )
            })
            .collect(),
        Screen::Dressing => vec![("Done", MenuAction::Back)],
        Screen::ContractOffer => vec![
            ("Accept contract", MenuAction::AcceptContract),
            ("Leave it for now", MenuAction::Back),
        ],
        Screen::ContractJournal => vec![("Back", MenuAction::Back)],
        Screen::Controls => vec![("Back", MenuAction::Back)],
        Screen::Settings => vec![
            ("Volume down", MenuAction::Volume(-1)),
            ("Volume up", MenuAction::Volume(1)),
            ("Shadows", MenuAction::Shadows),
            ("Back", MenuAction::Back),
        ],
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
    if keys.just_pressed(KeyCode::KeyM)
        && matches!(menu.screen(), Screen::Playing | Screen::Mapping)
    {
        actions.write(if menu.screen() == Screen::Playing {
            MenuAction::Map
        } else {
            MenuAction::Back
        });
        return;
    }
    if keys.just_pressed(KeyCode::KeyJ)
        && matches!(menu.screen(), Screen::Playing | Screen::ContractJournal)
    {
        actions.write(if menu.screen() == Screen::Playing {
            MenuAction::Journal
        } else {
            MenuAction::Back
        });
        return;
    }
    let identity = menu.screen() == Screen::Identity;
    if keys.just_pressed(KeyCode::Escape) || (!identity && keys.just_pressed(KeyCode::Backspace)) {
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
    if identity && keys.just_pressed(KeyCode::Tab) {
        let focus = menu.focus();
        menu.set_focus((focus + 1) % n);
    }
    let focus = menu.focus();
    if keys.just_pressed(KeyCode::ArrowUp) || (!identity && keys.just_pressed(KeyCode::KeyW)) {
        menu.set_focus((focus + n - 1) % n);
    } else if keys.just_pressed(KeyCode::ArrowDown)
        || (!identity && keys.just_pressed(KeyCode::KeyS))
    {
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
    } else if (keys.just_pressed(KeyCode::Enter)
        || (!identity && keys.any_just_pressed([KeyCode::NumpadEnter, KeyCode::Space])))
        && let Some(action) = menu.press()
    {
        actions.write(action);
    }
}
#[allow(clippy::too_many_arguments)]
pub fn actions(
    mut actions: MessageReader<MenuAction>,
    screen: Res<State<Screen>>,
    mut menu: ResMut<Menu>,
    mut next: ResMut<NextState<Screen>>,
    wardrobe: Res<Wardrobe>,
    mut players: Query<&mut Outfit, With<Player>>,
    mut exit: MessageWriter<AppExit>,
    mut identity: ResMut<IdentityPanel>,
    mut jobs: Option<ResMut<super::contracts::Contracts>>,
    mut settings: ResMut<super::settings::Settings>,
    mut shadows: ResMut<crate::shading::ShadowQuality>,
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
        if let MenuAction::IdentityAct(action) = action
            && menu.screen() == Screen::Identity
        {
            identity.act(*action);
        }
        if matches!(action, MenuAction::AcceptContract) && menu.screen() == Screen::ContractOffer {
            let Some(jobs) = jobs.as_mut() else {
                continue;
            };
            if !jobs.accept() {
                continue;
            }
        }
        if menu.screen() == Screen::Settings {
            match action {
                MenuAction::Volume(step) => settings.adjust_volume(*step),
                MenuAction::Shadows => super::settings::next_shadow(&mut shadows),
                _ => {}
            }
        }
        menu.apply(*action, &wardrobe, &mut outfit);
    }
    if menu.screen() != *screen.get() {
        next.set(menu.screen());
    }
}
