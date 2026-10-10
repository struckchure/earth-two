//! The first sponsored work order, from game/contracts.go. Records last this session.
use super::{GameAssets, Screen, Session, menu::Menu, navigation::MapPointer, seats::Seated};
use crate::{
    character::{Anim, CharacterController, Controls, Intent, Player, Traversal},
    landfall::{maps::WorldMap, terrain::stand_on},
    physics::set_paused,
    vehicle::{Drivable, Driving, Prompt, REACH},
    world::{KitAsset, LayoutAsset},
};
use avian3d::prelude::Physics;
use bevy::prelude::*;
use earth_two_world::{
    kit::{Kit, Placement},
    terrain::ARRIVAL,
};

pub const PASSAGE_DEBT: i32 = 2000;
pub const FIRST_PAY: i32 = 150;
pub const USE_REACH: f32 = 1.4;
pub const HISTORY_ROWS: usize = 4;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractState {
    Available,
    Accepted,
    Delivered,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub id: String,
    pub title: String,
    pub poster: String,
    pub objective: String,
    pub pay: i32,
    pub debt_credit: i32,
}
pub fn first_work_order() -> Record {
    Record {
        id: "PAD-001".into(),
        title: "First filing".into(),
        poster: "Ada Vellér".into(),
        objective: "Hand over the sealed arrival filing at the Exchange counter.".into(),
        pay: FIRST_PAY,
        debt_credit: 0,
    }
}
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Contracts {
    pub state: ContractState,
    pub balance: i32,
    pub debt: i32,
    pub ongoing: Option<Record>,
    pub completed: Vec<Record>,
    pub offer: Vec3,
    pub delivery: Vec3,
}
impl Contracts {
    pub fn new(kit: &Kit, placed: &[Placement]) -> Result<Self, String> {
        let point = |piece: &str, near: Vec3| {
            let mut best = 80.;
            let mut at = None;
            for p in placed.iter().filter(|p| p.piece == piece) {
                let origin = Vec3::from(p.at) + Vec3::Y * stand_on(kit, p);
                let distance = origin.xz().distance(near.xz());
                if distance < best {
                    best = distance;
                    at = Some(
                        origin
                            + Quat::from_rotation_y(p.turns as f32 * std::f32::consts::FRAC_PI_2)
                                * Vec3::Z
                                * 0.95,
                    );
                }
            }
            at.ok_or_else(|| format!("first contract: no {piece} near {near:?}"))
        };
        Ok(Self {
            state: ContractState::Available,
            balance: 0,
            debt: PASSAGE_DEBT,
            ongoing: None,
            completed: vec![],
            offer: point("terminal_kiosk", ARRIVAL)?,
            delivery: point("registrar_counter", Vec3::new(0., 0., -4.))?,
        })
    }
    pub fn available(&self) -> bool {
        self.state == ContractState::Available
    }
    pub fn accept(&mut self) -> bool {
        if !self.available() || self.ongoing.is_some() {
            return false;
        }
        self.state = ContractState::Accepted;
        self.ongoing = Some(first_work_order());
        true
    }
    pub fn deliver(&mut self, feet: Vec3) -> bool {
        if self.state != ContractState::Accepted
            || self.ongoing.is_none()
            || feet.distance(self.delivery) > USE_REACH
        {
            return false;
        }
        let mut receipt = self.ongoing.take().unwrap();
        receipt.debt_credit = self.debt.min(receipt.pay);
        self.debt -= receipt.debt_credit;
        self.completed.push(receipt);
        self.state = ContractState::Delivered;
        true
    }
    pub fn target(&self) -> Option<(Vec2, &'static str)> {
        match self.state {
            ContractState::Available => Some((self.offer.xz(), "Arrivals terminal")),
            ContractState::Accepted => Some((self.delivery.xz(), "Exchange delivery")),
            ContractState::Delivered => None,
        }
    }
    pub fn prompt(&self, feet: Vec3) -> &'static str {
        match self.state {
            ContractState::Available if feet.distance(self.offer) <= USE_REACH => {
                "Review Ada's contract"
            }
            ContractState::Accepted if feet.distance(self.delivery) <= USE_REACH => {
                "Hand over the sealed filing"
            }
            _ => "",
        }
    }
}
/// Guidance is derived, never written into the player's manual map marker.
pub fn route(map: &WorldMap, jobs: Option<&Contracts>) -> Option<(Vec2, &'static str)> {
    if map.marked {
        Some((map.dest, "Destination"))
    } else {
        jobs.and_then(Contracts::target)
    }
}
pub(super) fn load(
    mut commands: Commands,
    game: Res<GameAssets>,
    kits: Res<Assets<KitAsset>>,
    layouts: Res<Assets<LayoutAsset>>,
    jobs: Option<Res<Contracts>>,
    mut session: ResMut<Session>,
) {
    if jobs.is_some() || session.error.is_some() {
        return;
    }
    let (Some(kit), Some(layout)) = (kits.get(&game.kit), layouts.get(&game.layout)) else {
        return;
    };
    match Contracts::new(&kit.0, &layout.0.pieces) {
        Ok(jobs) => {
            commands.insert_resource(jobs);
        }
        Err(error) => session.error = Some(error),
    }
}
/// Runs after vehicle offers/entry and before character actions. Pauses immediately,
/// including the frame before Bevy applies the requested screen transition.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn interact(
    mut players: Query<
        (&mut Intent, &Transform, &CharacterController, &Traversal),
        (With<Player>, Without<Seated>),
    >,
    cars: Query<(&Drivable, &Transform)>,
    mut jobs: Option<ResMut<Contracts>>,
    mut menu: ResMut<Menu>,
    mut next: ResMut<NextState<Screen>>,
    mut controls: ResMut<Controls>,
    driving: Res<Driving>,
    mut physics: ResMut<Time<Physics>>,
    mut prompt: ResMut<Prompt>,
) {
    if menu.screen() != Screen::Playing || !controls.enabled || driving.active() {
        return;
    }
    let Some(jobs) = jobs.as_mut() else {
        return;
    };
    let Ok((mut intent, tr, cc, traversal)) = players.single_mut() else {
        return;
    };
    if !cc.grounded || traversal.active() {
        return;
    }
    let feet = tr.translation - Vec3::Y * cc.height / 2.;
    if cars
        .iter()
        .any(|(d, tr)| d.driver.is_none() && tr.translation.distance(feet) < REACH + 2.)
    {
        return;
    }
    let text = jobs.prompt(feet);
    if text.is_empty() {
        return;
    }
    if prompt.key.is_empty() {
        prompt.key = "E".into();
        prompt.text = text.into();
    }
    if intent.act != Anim::Interact {
        return;
    }
    if jobs.available() {
        menu.open(Screen::ContractOffer);
    } else if jobs.deliver(feet) {
        menu.open(Screen::ContractJournal);
        menu.contract_stamp = Some(ContractStamp {
            screen: Screen::ContractJournal,
            age: 0.,
        });
    } else {
        return;
    }
    *intent = default();
    controls.enabled = false;
    set_paused(&mut physics, true);
    next.set(menu.screen());
}
#[derive(Clone, Copy, Debug)]
pub struct ContractStamp {
    pub screen: Screen,
    pub age: f32,
}

/// Source journal geometry, shared by drawing and the receipt scroll hit-test.
pub fn journal_layout(size: Vec2) -> (f32, Rect, Rect) {
    let sc = super::navigation::scale(size)
        .min(size.x / 1072.)
        .min(size.y / 712.);
    let panel = Rect::from_center_size(size / sc / 2., Vec2::new(1040., 680.));
    let at = panel.min + Vec2::new(548., 194.);
    (
        sc,
        panel,
        Rect::from_corners(at, at + Vec2::new(464., 266.)),
    )
}
pub fn account_rects(size: Vec2) -> (Rect, Rect) {
    let sc = super::navigation::scale(size);
    let w = crate::landfall::maps::MINIMAP_SIZE * sc + 2. * (3. * sc).max(1.);
    let at = Vec2::new(size.x - 16. * sc - w, 16. * sc);
    (
        Rect::from_corners(at, at + Vec2::new(w, 50. * sc)),
        Rect::from_center_size(at, Vec2::splat(18. * sc)),
    )
}
pub(super) fn pointer(
    pointer: Res<MapPointer>,
    mut menu: ResMut<Menu>,
    jobs: Option<Res<Contracts>>,
    mut actions: MessageWriter<super::MenuAction>,
) {
    let Some(jobs) = jobs else {
        return;
    };
    if menu.screen() == Screen::Playing
        && pointer.pressed
        && let Some(at) = pointer.at
    {
        let (card, badge) = account_rects(pointer.size);
        if card.contains(at) || badge.contains(at) {
            actions.write(super::MenuAction::Journal);
        }
    }
    if menu.screen() == Screen::ContractJournal {
        let (sc, _, history) = journal_layout(pointer.size);
        let page = menu.stack.last_mut().unwrap();
        if pointer.at.is_some_and(|p| history.contains(p / sc)) {
            if pointer.wheel < 0. {
                page.receipt = page.receipt.saturating_add(1);
            }
            if pointer.wheel > 0. {
                page.receipt = page.receipt.saturating_sub(1);
            }
        }
        page.receipt = page
            .receipt
            .min(jobs.completed.len().saturating_sub(HISTORY_ROWS));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seated_and_driving_players_cannot_review_contracts() {
        for seated in [false, true] {
            let mut app = App::new();
            let root = crate::physics::source_assets();
            let jobs = Contracts::new(
                &Kit::load(&root, "world/world.json").unwrap(),
                &earth_two_world::kit::Layout::load(&root, "world/landfall.json")
                    .unwrap()
                    .pieces,
            )
            .unwrap();
            let feet = jobs.offer;
            let mut menu = Menu::default();
            menu.stack.clear();
            app.add_plugins(MinimalPlugins)
                .insert_resource(jobs)
                .insert_resource(menu)
                .insert_resource(Controls { enabled: true })
                .init_resource::<Driving>()
                .init_resource::<Prompt>()
                .init_resource::<Time<Physics>>()
                .init_resource::<NextState<Screen>>()
                .add_systems(Update, interact);
            let player = app
                .world_mut()
                .spawn((
                    Player,
                    Intent {
                        act: Anim::Interact,
                        ..default()
                    },
                    Transform::from_translation(feet + Vec3::Y * 0.9),
                    CharacterController {
                        grounded: true,
                        ..CharacterController::standing(0.3, 1.8, 0.3)
                    },
                    Traversal::default(),
                ))
                .id();
            if seated {
                app.world_mut().entity_mut(player).insert(Seated {
                    vehicle: player,
                    anim: Anim::Sitting,
                    hidden: false,
                    hands: vec![],
                    feet: vec![],
                });
            } else {
                app.world_mut().resource_mut::<Driving>().vehicle = Some(player);
            }
            app.update();
            assert_eq!(app.world().resource::<Menu>().screen(), Screen::Playing);
            assert!(app.world().resource::<Contracts>().available());
        }
    }
}
