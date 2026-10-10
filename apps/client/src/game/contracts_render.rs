//! PAD-001 offer, personal ledger and account HUD from contracts_ui.go / ledger.go.
use super::{
    MenuAction, Screen, Session,
    contracts::{self, Contracts, HISTORY_ROWS, PASSAGE_DEBT},
    menu::Menu,
    paper::*,
    render::Choice,
};
use crate::vehicle::Driving;
use bevy::{prelude::*, window::PrimaryWindow};
const HEAD: Color = Color::srgb_u8(219, 216, 195);
const DEBIT: Color = Color::srgb_u8(137, 71, 44);
const CREDIT: Color = Color::srgb_u8(49, 91, 74);
const PANEL: Color = Color::srgba_u8(16, 13, 11, 214);
const ACCENT: Color = Color::srgb_u8(255, 184, 82);
const LIGHT: Color = Color::srgb_u8(245, 240, 232);
#[derive(Component)]
pub(super) struct ContractRoot;
#[derive(PartialEq)]
pub(super) struct DrawKey {
    screen: Screen,
    focus: usize,
    offset: usize,
    size: Vec2,
    jobs: Contracts,
    hint: String,
    stamp: Option<(Screen, u32)>,
}
fn marks(n: i32) -> String {
    if n < 1000 {
        n.to_string()
    } else {
        format!("{},{:03}", n / 1000, n % 1000)
    }
}
#[derive(Component)]
pub(super) struct FitText(f32);
/// Go's textIn shrinks text to the measured available width. Use Bevy's
/// actual font layout, including Unicode names and proportional ledger fonts.
pub(super) fn fit(mut text: Query<(&FitText, &bevy::text::TextLayoutInfo, &mut TextFont)>) {
    for (width, layout, mut font) in &mut text {
        let measured = layout.size.x / layout.scale_factor;
        if measured > width.0 + 0.5
            && let bevy::text::FontSize::Px(size) = font.font_size
        {
            font.font_size = px((size * width.0 / measured).max(1.)).into();
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn label(
    c: &mut ChildSpawnerCommands,
    r: Rect,
    value: &str,
    font: &Handle<Font>,
    size: f32,
    ink: Color,
    sc: f32,
) {
    line(c, r, value, font, size, ink, false, sc);
}
fn box_at(at: Vec2, w: f32, h: f32) -> Rect {
    Rect::from_corners(at, at + Vec2::new(w, h))
}
#[allow(clippy::too_many_arguments)]
fn line(
    c: &mut ChildSpawnerCommands,
    r: Rect,
    value: &str,
    font: &Handle<Font>,
    size: f32,
    ink: Color,
    right: bool,
    sc: f32,
) {
    c.spawn((
        Node {
            justify_content: if right {
                JustifyContent::End
            } else {
                JustifyContent::Start
            },
            ..node(r.min.x, r.min.y, r.width(), r.height(), sc)
        },
        Pickable::IGNORE,
    ))
    .with_child((text(value, font, size * sc, ink), FitText(r.width() * sc)));
}
fn offer_panel(size: Vec2, sc: f32) -> Rect {
    let at = Vec2::new(
        ((size.x / sc - 540.) / 2.).max(16.),
        ((size.y / sc - 580.) / 2.).max(24.),
    );
    box_at(at, 540., 580.)
}
fn offer(
    c: &mut ChildSpawnerCommands,
    r: Rect,
    focus: usize,
    interactive: bool,
    f: &Fonts,
    sc: f32,
) {
    sheet(c, r, sc);
    let at = r.min + Vec2::splat(28.);
    let w = r.width() - 56.;
    // The contract's issuer differs from the other Exchange forms.
    label(
        c,
        box_at(at, w - 75., 16.),
        "THE EXCHANGE  •  CONTRACT FOR FILING",
        &f.label,
        11.,
        MUTED,
        sc,
    );
    c.spawn((
        Node {
            border: UiRect::all(px(sc.max(1.))),
            ..node(at.x + w - 66., at.y - 2., 66., 20., sc)
        },
        BorderColor::all(INK),
        Pickable::IGNORE,
    ))
    .with_child(text("PAD-001", &f.bold, 11. * sc, INK));
    label(
        c,
        box_at(at + Vec2::Y * 22., w, 37.5),
        "First filing",
        &f.heading,
        30.,
        INK,
        sc,
    );
    for y in [65.5, 68.5] {
        rect(c, box_at(at + Vec2::Y * y, w, 1.), INK, sc);
    }
    let at = at + Vec2::Y * 77.5;
    label(
        c,
        box_at(at, w, 18.),
        "Day labour • posted at Patience arrivals, the Pads",
        &f.label,
        12.,
        MUTED,
        sc,
    );
    rect(c, box_at(at + Vec2::Y * 28., 2., 48.), RULE, sc);
    for (dy, value) in [
        (28., "“Welcome. Your passage is on my books: 2,000 marks."),
        (
            52.,
            "Deliver this filing, and I will credit the first 150.”",
        ),
    ] {
        label(
            c,
            box_at(at + Vec2::new(14., dy), w - 14., 22.),
            value,
            &f.typed,
            15.,
            INK,
            sc,
        );
    }
    for (dy, h, value, font, size, color) in [
        (90., 14., "The work", &f.label, 10., MUTED),
        (
            104.,
            20.,
            "Carry a sealed arrival filing from the Pads to Landfall, and",
            &f.typed,
            14.,
            INK,
        ),
        (
            124.,
            20.,
            "hand it over at the marked Registrar counter.",
            &f.typed,
            14.,
            INK,
        ),
    ] {
        label(
            c,
            box_at(at + Vec2::Y * dy, w, h),
            value,
            font,
            size,
            color,
            sc,
        );
    }
    let half = (w - 16.) / 2.;
    for (i, (name, value)) in [
        ("Pay", "150 marks off the debt"),
        ("Bond", "None"),
        ("Deadline", "None"),
        ("Penalty on default", "None"),
    ]
    .iter()
    .enumerate()
    {
        let p = at + Vec2::new((i % 2) as f32 * (half + 16.), 156. + (i / 2) as f32 * 44.);
        label(c, box_at(p, half, 14.), name, &f.label, 10., MUTED, sc);
        label(
            c,
            box_at(p + Vec2::Y * 14., half, 24.),
            value,
            &f.typed,
            15.,
            INK,
            sc,
        );
        rect(c, box_at(p + Vec2::Y * 38., half, 1.), RULE, sc);
    }
    let sy = at + Vec2::Y * 252.;
    label(c, box_at(sy, half, 14.), "Poster", &f.label, 10., MUTED, sc);
    label(
        c,
        box_at(sy + Vec2::new(6., 12.), half, 26.),
        "A. Vellér",
        &f.bold,
        22.,
        Color::srgb_u8(38, 58, 128),
        sc,
    );
    rect(c, box_at(sy + Vec2::Y * 40., half * 0.8, 1.), INK, sc);
    label(
        c,
        box_at(sy + Vec2::new(half + 16., 6.), half - 70., 30.),
        "Licence: Unlisted and up",
        &f.typed,
        12.,
        MUTED,
        sc,
    );
    seal(c, sy + Vec2::new(w - 30., 22.), f, sc);
    for (i, (name, action)) in super::menu::choices(Screen::ContractOffer)
        .into_iter()
        .enumerate()
    {
        let p = Vec2::new(r.min.x + 28., r.max.y - 138. + i as f32 * 60.);
        choice(
            c,
            box_at(p, w, 50.),
            name,
            focus == i,
            interactive.then_some(Choice { action, focus: i }),
            f,
            sc,
        );
    }
}
struct Cell {
    value: String,
    note: String,
    ink: Color,
}
impl Cell {
    fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            note: String::new(),
            ink: INK,
        }
    }
    fn note(mut self, note: impl Into<String>) -> Self {
        self.note = note.into();
        self
    }
    fn credit(value: i32) -> Self {
        Self {
            ink: CREDIT,
            ..Self::new(marks(value))
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn table(
    c: &mut ChildSpawnerCommands,
    r: Rect,
    columns: &[(&str, f32, bool)],
    rows: &[Vec<Cell>],
    totals: &[Cell],
    empty: &str,
    f: &Fonts,
    sc: f32,
) {
    rect(c, r, PAPER, sc);
    rect(c, box_at(r.min, r.width(), 28.), HEAD, sc);
    let foot = if totals.is_empty() { 0. } else { 30. };
    let fy = r.max.y - foot;
    if foot > 0. {
        rect(c, Rect::new(r.min.x, fy, r.max.x, r.max.y), HEAD, sc);
    }
    let mut x = r.min.x;
    for (col, (name, share, right)) in columns.iter().enumerate() {
        let width = r.width() * share;
        let cell = |y, h| Rect::new(x + 10., y, x + width - 10., y + h);
        line(
            c,
            cell(r.min.y, 28.),
            name,
            &f.label,
            10.,
            MUTED,
            *right,
            sc,
        );
        for (i, row) in rows.iter().enumerate() {
            let Some(v) = row.get(col) else { continue };
            let y = r.min.y + 28. + i as f32 * 52.;
            if v.note.is_empty() {
                line(c, cell(y, 52.), &v.value, &f.label, 14., v.ink, *right, sc);
            } else {
                line(
                    c,
                    cell(y + 5., 24.),
                    &v.value,
                    &f.label,
                    14.,
                    v.ink,
                    *right,
                    sc,
                );
                line(
                    c,
                    cell(y + 29., 18.),
                    &v.note,
                    &f.regular,
                    11.,
                    MUTED,
                    *right,
                    sc,
                );
            }
        }
        if let Some(v) = totals.get(col) {
            line(c, cell(fy, 30.), &v.value, &f.label, 13., v.ink, *right, sc);
        }
        rect(c, Rect::new(x, r.min.y, x + 1., r.max.y), RULE, sc);
        x += width;
    }
    let mut y = r.min.y + 28.;
    while y < fy {
        rect(c, Rect::new(r.min.x, y, r.max.x, y + 1.), RULE, sc);
        y += 52.;
    }
    for y in [r.min.y, r.max.y - 1.] {
        rect(c, Rect::new(r.min.x, y, r.max.x, y + 1.), RULE, sc);
    }
    if foot > 0. {
        for y in [fy, fy + 3.] {
            rect(c, Rect::new(r.min.x, y, r.max.x, y + 1.), RULE, sc);
        }
    }
    rect(
        c,
        Rect::new(r.max.x - 1., r.min.y, r.max.x, r.max.y),
        RULE,
        sc,
    );
    if rows.is_empty() {
        let r = Rect::new(r.min.x + 1., r.min.y + 29., r.max.x - 1., r.min.y + 80.);
        rect(c, r, PAPER, sc);
        label(c, r.inflate(-10.), empty, &f.regular, 13., MUTED, sc);
    }
}
fn journal(
    c: &mut ChildSpawnerCommands,
    r: Rect,
    jobs: &Contracts,
    offset: usize,
    f: &Fonts,
    sc: f32,
) {
    rect(c, r.inflate(5.), Color::srgb_u8(41, 35, 27), sc);
    rect(c, r, PAPER, sc);
    rect(
        c,
        Rect::new(r.center().x - 6., r.min.y, r.center().x + 6., r.max.y),
        Color::srgba_u8(103, 91, 65, 25),
        sc,
    );
    let left = r.min + Vec2::splat(28.);
    let right = left + Vec2::X * 520.;
    for x in [left.x - 8., right.x - 8.] {
        rect(
            c,
            Rect::new(x, r.min.y + 150., x + 1., r.max.y - 40.),
            Color::srgba_u8(151, 82, 66, 95),
            sc,
        );
    }
    for (dy, h, v, font, size, ink) in [
        (0., 20., "PERSONAL LEDGER • UNLISTED", &f.label, 10., MUTED),
        (27., 40., "Account book", &f.heading, 32., INK),
        (
            78.,
            22.,
            "Debts owed and work on record.",
            &f.regular,
            14.,
            MUTED,
        ),
    ] {
        label(
            c,
            box_at(left + Vec2::Y * dy, 464., h),
            v,
            font,
            size,
            ink,
            sc,
        );
    }
    label(
        c,
        box_at(right, 464., 20.),
        "ACCOUNT TOTALS • MARKS",
        &f.label,
        10.,
        MUTED,
        sc,
    );
    for (i, (name, amount, ink)) in [
        ("Balance", jobs.balance, INK),
        ("Debt owed", jobs.debt, DEBIT),
    ]
    .iter()
    .enumerate()
    {
        let p = right + Vec2::new(i as f32 * 232., 29.);
        label(
            c,
            box_at(p + Vec2::X * 12., 208., 20.),
            name,
            &f.regular,
            11.,
            MUTED,
            sc,
        );
        line(
            c,
            box_at(p + Vec2::new(12., 23.), 208., 34.),
            &marks(*amount),
            &f.label,
            26.,
            *ink,
            true,
            sc,
        );
    }
    for x in [right.x, right.x + 232., right.x + 463.] {
        rect(
            c,
            Rect::new(x, right.y + 29., x + 1., right.y + 91.),
            RULE,
            sc,
        );
    }
    for y in [right.y + 29., right.y + 91.] {
        rect(c, Rect::new(right.x, y, right.x + 464., y + 1.), RULE, sc);
    }
    let debts = box_at(r.min + Vec2::new(28., 174.), 464., 110.);
    let available = box_at(r.min + Vec2::new(28., 358.), 464., 80.);
    let ongoing = box_at(r.min + Vec2::new(28., 514.), 464., 96.);
    let history = box_at(r.min + Vec2::new(548., 194.), 464., 266.);
    let section = |c: &mut ChildSpawnerCommands, name: &str, count: &str, r: Rect| {
        label(
            c,
            box_at(r.min - Vec2::Y * 32., r.width(), 26.),
            name,
            &f.label,
            18.,
            INK,
            sc,
        );
        line(
            c,
            box_at(r.min + Vec2::new(r.width() - 160., -34.), 160., 26.),
            count,
            &f.regular,
            11.,
            MUTED,
            true,
            sc,
        );
    };
    section(c, "Debts owed", "Marks", debts);
    let repaid = (PASSAGE_DEBT - jobs.debt).max(0);
    table(
        c,
        debts,
        &[
            ("CREDITOR / DEBT", 0.4, false),
            ("ORIGINAL", 0.2, true),
            ("REPAID", 0.18, true),
            ("DUE", 0.22, true),
        ],
        &[vec![
            Cell::new("Ada Vellér").note("Passage on Patience"),
            Cell::new(marks(PASSAGE_DEBT)),
            Cell::credit(repaid),
            Cell {
                ink: DEBIT,
                ..Cell::new(marks(jobs.debt))
            },
        ]],
        &[
            Cell::new("Total"),
            Cell::new(marks(PASSAGE_DEBT)),
            Cell::credit(repaid),
            Cell {
                ink: DEBIT,
                ..Cell::new(marks(jobs.debt))
            },
        ],
        "",
        f,
        sc,
    );
    label(
        c,
        box_at(debts.min + Vec2::Y * 122., 464., 22.),
        "Work credits repay the passage debt.",
        &f.regular,
        12.,
        MUTED,
        sc,
    );
    section(
        c,
        "Available contracts",
        if jobs.available() {
            "1 notice"
        } else {
            "0 notices"
        },
        available,
    );
    let notices = if jobs.available() {
        vec![vec![
            Cell::new("Arrivals terminal").note("The Pads"),
            Cell::credit(1),
        ]]
    } else {
        vec![]
    };
    table(
        c,
        available,
        &[
            ("TERMINAL / LOCATION", 0.75, false),
            ("AVAILABLE", 0.25, true),
        ],
        &notices,
        &[],
        "No contract notifications.",
        f,
        sc,
    );
    if jobs.available() {
        label(
            c,
            box_at(available.min + Vec2::Y * 90., 464., 20.),
            "Visit the terminal to read the terms and accept.",
            &f.regular,
            12.,
            MUTED,
            sc,
        );
    }
    section(
        c,
        "Ongoing contract",
        if jobs.ongoing.is_some() {
            "1 / 1 active"
        } else {
            "0 / 1 active"
        },
        ongoing,
    );
    rect(c, ongoing, HEAD, sc);
    let mut ln = |value: &str, dy, size, ink| {
        label(
            c,
            box_at(ongoing.min + Vec2::new(12., dy), 440., size + 6.),
            value,
            &f.regular,
            size,
            ink,
            sc,
        )
    };
    if let Some(order) = &jobs.ongoing {
        ln(&order.title, 9., 18., INK);
        ln(&format!("{} • {}", order.id, order.poster), 30., 11., MUTED);
        ln(&order.objective, 50., 13., INK);
        ln(
            &format!("Pay: {} marks to debt • E to hand over", marks(order.pay)),
            73.,
            12.,
            CREDIT,
        );
    } else {
        ln("No ongoing contract", 17., 17., INK);
        ln("One contract at a time.", 50., 12., MUTED);
    }
    let start = offset.min(jobs.completed.len().saturating_sub(HISTORY_ROWS));
    let visible = HISTORY_ROWS.min(jobs.completed.len() - start);
    let count = if jobs.completed.len() > HISTORY_ROWS {
        format!(
            "{}-{} of {}",
            start + 1,
            start + visible,
            jobs.completed.len()
        )
    } else {
        format!(
            "{} receipt{}",
            jobs.completed.len(),
            if jobs.completed.len() == 1 { "" } else { "s" }
        )
    };
    section(c, "Completed contracts", &count, history);
    let rows: Vec<_> = jobs
        .completed
        .iter()
        .rev()
        .skip(start)
        .take(HISTORY_ROWS)
        .map(|r| {
            vec![
                Cell::new(&r.id),
                Cell::new(&r.title).note(&r.poster),
                Cell::credit(r.debt_credit),
                Cell {
                    ink: CREDIT,
                    ..Cell::new("Delivered")
                },
            ]
        })
        .collect();
    table(
        c,
        history,
        &[
            ("REF.", 0.15, false),
            ("CONTRACT / PAYER", 0.43, false),
            ("DEBT CREDIT", 0.23, true),
            ("STATUS", 0.19, false),
        ],
        &rows,
        &[
            Cell::new(""),
            Cell::new("Total credited"),
            Cell::credit(jobs.completed.iter().map(|r| r.debt_credit).sum()),
            Cell::new(""),
        ],
        "No completed contracts yet.",
        f,
        sc,
    );
    label(
        c,
        box_at(history.min + Vec2::Y * 278., 464., 22.),
        "Receipt credits reduce debt; they do not add cash.",
        &f.regular,
        12.,
        MUTED,
        sc,
    );
    let hint = if jobs.completed.len() > HISTORY_ROWS {
        "All amounts in marks • Scroll receipts to browse"
    } else {
        "All amounts in marks"
    };
    label(
        c,
        box_at(r.min + Vec2::new(28., 602.), 784., 50.),
        hint,
        &f.regular,
        12.,
        MUTED,
        sc,
    );
    let p = r.min + Vec2::new(832., 602.);
    c.spawn((
        node(p.x, p.y, 180., 50., sc),
        BackgroundColor(INK),
        bevy::ui_widgets::Button,
        bevy::ui_widgets::ActivateOnPress,
        Choice {
            action: MenuAction::Back,
            focus: 0,
        },
    ))
    .with_child(text("Back", &f.label, 17. * sc, PAPER));
}
fn smooth(a: f32, b: f32, t: f32) -> f32 {
    let x = ((t - a) / (b - a)).clamp(0., 1.);
    x * x * (3. - 2. * x)
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn draw(
    mut commands: Commands,
    screen: Res<State<Screen>>,
    mut menu: ResMut<Menu>,
    jobs: Option<Res<Contracts>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    fonts: Res<Fonts>,
    time: Res<Time>,
    session: Res<Session>,
    transforms: Query<&Transform>,
    driving: Res<Driving>,
    roots: Query<Entity, With<ContractRoot>>,
    mut previous: Local<Option<DrawKey>>,
) {
    let (Some(jobs), Ok(w)) = (jobs, windows.single()) else {
        return;
    };
    if let Some(stamp) = menu.contract_stamp.as_mut() {
        stamp.age += time.delta_secs();
        if stamp.age > 1.5 {
            menu.contract_stamp = None;
        }
    }
    // Interactions update the menu in this Update; Bevy applies NextState on
    // the next frame. Draw the new page now so SETTLED never lands on a blank sheet.
    let screen = if *screen.get() == Screen::Loading {
        Screen::Loading
    } else {
        menu.screen()
    };
    let size = Vec2::new(w.width(), w.height());
    let hint = if driving.active() {
        "Park outside the Hull; deliver on foot."
    } else if session
        .player
        .and_then(|e| transforms.get(e).ok())
        .is_some_and(|tr| tr.translation.xz().distance(jobs.delivery.xz()) > 100.)
    {
        "Follow the caravan road west to Landfall."
    } else {
        "Carrying: sealed arrival filing"
    };
    let key = DrawKey {
        screen,
        focus: menu.focus(),
        offset: menu.stack.last().map_or(0, |p| p.receipt),
        size,
        jobs: jobs.clone(),
        hint: hint.into(),
        stamp: menu.contract_stamp.map(|s| (s.screen, s.age.to_bits())),
    };
    if previous.as_ref() == Some(&key) {
        return;
    }
    let offset = key.offset;
    *previous = Some(key);
    for e in &roots {
        commands.entity(e).despawn();
    }
    if !matches!(
        screen,
        Screen::Playing | Screen::ContractOffer | Screen::ContractJournal
    ) && menu.contract_stamp.is_none()
    {
        return;
    }
    let sc = scale(size.x, size.y);
    commands
        .spawn((
            ContractRoot,
            node(0., 0., size.x, size.y, 1.),
            Pickable::IGNORE,
            GlobalZIndex(20),
        ))
        .with_children(|c| {
            match screen {
                Screen::ContractOffer => {
                    rect(
                        c,
                        box_at(Vec2::ZERO, size.x / sc, size.y / sc),
                        Color::srgba_u8(0, 0, 0, 110),
                        sc,
                    );
                    offer(c, offer_panel(size, sc), menu.focus(), true, &fonts, sc);
                }
                Screen::ContractJournal => {
                    rect(
                        c,
                        box_at(Vec2::ZERO, size.x, size.y),
                        Color::srgba_u8(0, 0, 0, 110),
                        1.,
                    );
                    let (sc, r, _) = contracts::journal_layout(size);
                    journal(c, r, &jobs, offset, &fonts, sc);
                }
                Screen::Playing => {
                    let (card, badge) = contracts::account_rects(size);
                    rect(c, card, PANEL, 1.);
                    for (i, (name, amount, ink)) in [
                        ("Balance", jobs.balance, LIGHT),
                        (
                            "Debt",
                            jobs.debt,
                            if jobs.debt > 0 { ACCENT } else { LIGHT },
                        ),
                    ]
                    .iter()
                    .enumerate()
                    {
                        let p = card.min / sc + Vec2::new(10., 4. + i as f32 * 23.);
                        label(
                            c,
                            box_at(p, 46., 19.),
                            name,
                            &fonts.label,
                            11.,
                            Color::srgb_u8(178, 168, 156),
                            sc,
                        );
                        line(
                            c,
                            box_at(p + Vec2::X * 48., card.width() / sc - 68., 19.),
                            &format!("{} marks", marks(*amount)),
                            &fonts.label,
                            14.,
                            *ink,
                            true,
                            sc,
                        );
                    }
                    if jobs.available() {
                        rect(c, badge.inflate(sc), PANEL, 1.);
                        rect(c, badge, ACCENT, 1.);
                        c.spawn((
                            node(badge.min.x, badge.min.y, badge.width(), badge.height(), 1.),
                            Pickable::IGNORE,
                        ))
                        .with_child(text(
                            "1",
                            &fonts.label,
                            12. * sc,
                            Color::srgb_u8(30, 24, 18),
                        ));
                    }
                    if let Some(order) = &jobs.ongoing {
                        let p = Vec2::new(
                            16.,
                            16. + crate::landfall::maps::MINIMAP_SIZE + 3. + 6. + 50. + 14.,
                        );
                        rect(c, box_at(p, 310., 102.), PANEL, sc);
                        for (dy, value, size, ink) in [
                            (0., order.title.as_str(), 19., ACCENT),
                            (30., "Accepted • deliver to the Exchange", 14., LIGHT),
                            (56., hint, 14., Color::srgb_u8(178, 168, 156)),
                        ] {
                            label(
                                c,
                                box_at(p + Vec2::new(14., 12. + dy), 282., 24.),
                                value,
                                &fonts.label,
                                size,
                                ink,
                                sc,
                            );
                        }
                    }
                }
                _ => {}
            }
            if let Some(stamp) = menu.contract_stamp {
                let (stamp_sc, mut r) = if stamp.screen == Screen::ContractJournal {
                    let (s, r, _) = contracts::journal_layout(size);
                    (s, r)
                } else {
                    (sc, offer_panel(size, sc))
                };
                let on_paper = screen == stamp.screen;
                if !on_paper {
                    let k = smooth(0.54, 1.5, stamp.age);
                    let away = k * k * (size.y / stamp_sc - r.min.y + 40.);
                    r.min.y += away;
                    r.max.y += away;
                    if stamp.screen == Screen::ContractOffer {
                        offer(c, r, 0, false, &fonts, stamp_sc);
                    } else {
                        sheet(c, r, stamp_sc);
                    }
                }
                let drop = smooth(0., 0.14, stamp.age);
                let zoom = 1. + 0.7 * (1. - drop);
                let alpha = drop
                    * if on_paper {
                        1. - smooth(1.05, 1.5, stamp.age)
                    } else {
                        1.
                    };
                let settled = stamp.screen == Screen::ContractJournal;
                let word = if settled { "SETTLED" } else { "FILED" };
                let ink = if settled {
                    Color::srgba(49. / 255., 91. / 255., 74. / 255., alpha * 0.9)
                } else {
                    Color::srgba(172. / 255., 38. / 255., 32. / 255., alpha * 0.9)
                };
                let at = Vec2::new(r.center().x, r.min.y + r.height() * 0.4);
                let width = word.len() as f32 * 34. * zoom + 32. * zoom;
                let height = 70. * zoom;
                let turn = (-8.
                    + 6. * earth_two_world::terrain::lattice(
                        word.len() as i32,
                        if settled { 7 } else { 6 },
                        0x5a3,
                    ))
                .to_radians();
                c.spawn((
                    node(
                        at.x - width / 2.,
                        at.y - height / 2.,
                        width,
                        height,
                        stamp_sc,
                    ),
                    UiTransform::from_rotation(Rot2::radians(turn)),
                    Pickable::IGNORE,
                ))
                .with_children(|c| {
                    for inset in [0., 7.36 * zoom] {
                        c.spawn((
                            Node {
                                border: UiRect::all(px(if inset == 0. { 5.06 } else { 2.3 }
                                    * zoom
                                    * stamp_sc)),
                                ..node(
                                    inset,
                                    inset,
                                    width - 2. * inset,
                                    height - 2. * inset,
                                    stamp_sc,
                                )
                            },
                            BorderColor::all(ink),
                            Pickable::IGNORE,
                        ));
                    }
                    c.spawn((node(0., 0., width, height, stamp_sc), Pickable::IGNORE))
                        .with_child(text(word, &fonts.heading, 46. * zoom * stamp_sc, ink));
                    for i in 0..(width * height / (81. * zoom * zoom)) as i32 {
                        let x = earth_two_world::terrain::lattice(i, 4, 0x57a9) * width;
                        let y = earth_two_world::terrain::lattice(i, 5, 0x57a9) * height;
                        let edge =
                            (1. + 1.5 * earth_two_world::terrain::lattice(i, 6, 0x57a9)) * zoom;
                        rect(
                            c,
                            Rect::new(x, y, x + edge, y + edge),
                            Color::srgba_u8(236, 227, 207, (200. * alpha) as u8),
                            stamp_sc,
                        );
                    }
                });
            }
        });
}
