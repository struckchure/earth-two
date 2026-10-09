//! Source-linked wardrobe and menu regressions from game/wardrobe_test.go.
use earth_two_client::{
    game::{MenuAction, Screen, menu::Menu, wardrobe::*},
    presentation::{
        Outfit, Slot, Wardrobe,
        outfit::{BodyWardrobe, Item, Look, Tone},
    },
};
fn wardrobe() -> Wardrobe {
    let mut man = BodyWardrobe {
        name: "man".into(),
        tones: vec![Tone::named("Dark"), Tone::named("Fair")],
        ..Default::default()
    };
    *man.items_mut(Slot::Top) = vec![Item::named("T-shirt"), Item::named("Polo")];
    *man.items_mut(Slot::OnePiece) = vec![Item::named("Suit & tie")];
    man.looks = vec![
        Look::new("Corvane", &[(Slot::OnePiece, 0)], &[]),
        Look::new("Casual", &[(Slot::Top, 1)], &[Slot::OnePiece]),
    ];
    let mut woman = BodyWardrobe {
        name: "woman".into(),
        tones: vec![Tone::named("Dark")],
        ..Default::default()
    };
    *woman.items_mut(Slot::Top) = vec![Item::named("Tank top"), Item::named("T-shirt")];
    Wardrobe {
        bodies: vec![man, woman],
    }
}
#[test]
fn slot_cycles_through_none_and_one_piece_replaces_separates() {
    let w = wardrobe();
    let mut o = Outfit::default();
    let top = ROW_SLOTS + Slot::Top.index();
    for expected in ["T-shirt", "Polo", "None", "T-shirt"] {
        o = cycle_row(&w, o, top, 1);
        assert_eq!(row_text(&w, o, top).1, expected);
    }
    o = cycle_row(&w, o, top, -1);
    assert_eq!(row_text(&w, o, top).1, "None");
    o = cycle_row(&w, o, top, 1);
    o = cycle_row(&w, o, ROW_SLOTS + Slot::OnePiece.index(), 1);
    assert!(o.item(Slot::Top).is_none());
    o = cycle_row(&w, o, top, 1);
    assert!(o.item(Slot::OnePiece).is_none());
}
#[test]
fn body_switch_preserves_names_and_clamps_tone() {
    let w = wardrobe();
    let mut o = cycle_row(&w, Outfit::default(), ROW_TONE, 1);
    assert_eq!(row_text(&w, o, ROW_TONE).1, "Fair");
    o = cycle_row(&w, o, ROW_SLOTS + Slot::Top.index(), 1);
    o = cycle_row(&w, o, ROW_BODY, 1);
    assert_eq!(row_text(&w, o, ROW_BODY), ("Body".into(), "Woman".into()));
    assert_eq!(row_text(&w, o, ROW_TONE).1, "Dark");
    assert_eq!(o.item(Slot::Top), Some(1));
    o = cycle_row(&w, o, ROW_BODY, 1);
    assert_eq!(row_text(&w, o, ROW_BODY).1, "Man");
    assert_eq!(o.item(Slot::Top), Some(0));
}
#[test]
fn looks_wrap_and_custom_clothes_are_own() {
    let w = wardrobe();
    let mut o = Outfit::default();
    assert_eq!(row_text(&w, o, ROW_LOOK).1, "Own");
    assert_eq!(
        row_text(&w, cycle_row(&w, o, ROW_LOOK, -1), ROW_LOOK).1,
        "Casual"
    );
    for name in ["Corvane", "Casual", "Corvane"] {
        o = cycle_row(&w, o, ROW_LOOK, 1);
        assert_eq!(row_text(&w, o, ROW_LOOK).1, name);
    }
    assert_eq!(
        row_text(&w, o, ROW_SLOTS + Slot::OnePiece.index()).1,
        "Suit & tie"
    );
    o = cycle_row(&w, o, ROW_SLOTS + Slot::Top.index(), 1);
    assert_eq!(row_text(&w, o, ROW_LOOK).1, "Own");
}
#[test]
fn empty_rows_and_standard_face_are_preserved() {
    let w = wardrobe();
    let o = Outfit::new(1, 0);
    for row in [
        ROW_LOOK,
        ROW_SLOTS + Slot::Hair.index(),
        ROW_SLOTS + Slot::Face.index(),
    ] {
        assert_eq!(cycle_row(&w, o, row, 1), o);
    }
    assert_eq!(
        row_text(&w, o, ROW_SLOTS + Slot::Face.index()).1,
        "Standard"
    );
    assert_eq!(row_text(&w, o, ROW_SLOTS + Slot::Hair.index()).1, "None");
}
#[test]
fn wardrobe_returns_to_its_parent_with_focus_and_outfit_intact() {
    let w = wardrobe();
    let mut o = Outfit::default();
    let mut m = Menu::default();
    m.apply(MenuAction::Back, &w, &mut o);
    assert_eq!(m.screen(), Screen::Title);
    for parent in [Screen::Title, Screen::Paused] {
        if parent == Screen::Paused {
            m.apply(MenuAction::Play, &w, &mut o);
            m.apply(MenuAction::Pause, &w, &mut o);
        }
        m.set_focus(1);
        m.apply(m.press().unwrap(), &w, &mut o);
        assert_eq!(m.screen(), Screen::Dressing);
        assert_eq!(m.focus(), 0);
        assert_eq!(m.on_title(), parent == Screen::Title);
        let prior = o;
        m.apply(
            MenuAction::CycleRow {
                row: ROW_TONE,
                step: 1,
            },
            &w,
            &mut o,
        );
        assert_ne!(o, prior);
        m.set_focus(ROW_COUNT);
        m.apply(m.press().unwrap(), &w, &mut o);
        assert_eq!(m.screen(), parent);
        assert_eq!(m.focus(), 1);
    }
    m.apply(MenuAction::Back, &w, &mut o);
    assert_eq!(m.screen(), Screen::Playing);
    assert!(m.stack.is_empty());
}
#[test]
fn wardrobe_cues_track_depth_and_only_actual_outfit_changes() {
    use earth_two_client::game::ui_sound::UiSoundState;
    let mut sounds = UiSoundState::default();
    let o = Outfit::default();
    sounds.update(Screen::Title, 1, 1, Some(o), "");
    assert_eq!(
        sounds.update(Screen::Dressing, 2, 0, Some(o), "")[0].name,
        "ui_page"
    );
    let changed = Outfit::new(0, 1);
    assert_eq!(
        sounds.update(Screen::Dressing, 2, 0, Some(changed), "")[0].name,
        "cloth"
    );
    assert!(
        sounds
            .update(Screen::Dressing, 2, 0, Some(changed), "")
            .is_empty()
    );
    assert_eq!(
        sounds.update(Screen::Title, 1, 1, Some(changed), "")[0].name,
        "ui_back"
    );
}
