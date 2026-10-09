//! The in-game identity form and account connection, from game/identity.go:
//! an encrypted key the player creates, unlocks, imports and exports; a
//! connection that links it to the database; the optional email and display
//! name; and the latency probe behind the connection HUD. Drawing belongs
//! to the menus; this module owns the state, the messages and the jobs.

pub mod jobs;
pub mod storage;

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    time::Duration,
};

use bevy::prelude::*;
use earth_two_identity::{
    Key, PlatformTransport, Session, clock::Moment, document_account_id, import,
};

use jobs::MaybeSend;

pub const IDENTITY_FIELDS: usize = 4;
/// Byte limits per field: passphrase, transfer path, email, display name.
pub const FIELD_LIMITS: [usize; IDENTITY_FIELDS] = [4096, 1024, 254, 512];
pub const FIELD_LABELS: [&str; IDENTITY_FIELDS] = [
    "KEY PASSPHRASE",
    "KEY FILE (IMPORT / EXPORT)",
    "OPTIONAL CONTACT EMAIL",
    "DISPLAY NAME (ANY NAME; DOES NOT HAVE TO BE UNIQUE)",
];
pub const FOOTNOTE: &str =
    "Only your private key controls this account. Losing every backup means losing access.";
/// Whether the transfer path field is editable: a browser picks files itself.
pub const CAN_EDIT_TRANSFER: bool = cfg!(not(target_arch = "wasm32"));

pub type AccountSession = Session<PlatformTransport>;

/// The form's buttons, in the Go menu's order, plus Back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IdentityAction {
    Create,
    Unlock,
    Connect,
    Lock,
    Import,
    Export,
    Email,
    Name,
    Back,
}

impl IdentityAction {
    /// Button labels and actions, as `items(identityScreen)`.
    pub const ITEMS: [(&'static str, IdentityAction); 9] = [
        ("Create identity", IdentityAction::Create),
        ("Unlock key", IdentityAction::Unlock),
        ("Connect account", IdentityAction::Connect),
        ("Lock key", IdentityAction::Lock),
        ("Import key", IdentityAction::Import),
        ("Export key", IdentityAction::Export),
        ("Save email", IdentityAction::Email),
        ("Save display name", IdentityAction::Name),
        ("Back", IdentityAction::Back),
    ];
}

struct IdentityResult {
    backup: Option<Vec<u8>>,
    key: Option<Key>,
    session: Option<Arc<AccountSession>>,
    email: Option<String>,
    display_name: Option<String>,
    message: &'static str,
    err: Option<earth_two_identity::Error>,
}

impl IdentityResult {
    fn message(message: &'static str) -> IdentityResult {
        IdentityResult {
            backup: None,
            key: None,
            session: None,
            email: None,
            display_name: None,
            message,
            err: None,
        }
    }

    fn error(err: earth_two_identity::Error) -> IdentityResult {
        IdentityResult {
            err: Some(err),
            ..IdentityResult::message("")
        }
    }
}

struct LatencyResult {
    session: Arc<AccountSession>,
    latency: Result<Duration, earth_two_identity::Error>,
}

/// A frame's text editing for the focused field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextInput {
    pub backspace: bool,
    pub modifier: bool,
    pub select_all: bool,
    pub paste: bool,
    pub chars: Vec<char>,
}

#[derive(Resource)]
pub struct IdentityPanel {
    /// Passphrase, transfer path, email, display name.
    pub fields: [String; IDENTITY_FIELDS],
    pub host: String,
    pub database: String,
    pub backup: Vec<u8>,
    pub key: Option<Key>,
    pub session: Option<Arc<AccountSession>>,
    pub message: String,
    pub failed: bool,
    pub busy: bool,
    pub latency: Duration,
    /// The field the menu has focused, when the form is open; the menus own
    /// navigation and set this so `edit_from_input` can type into it.
    pub focus: Option<usize>,
    results: Mutex<Receiver<IdentityResult>>,
    results_tx: Sender<IdentityResult>,
    ping_results: Mutex<Receiver<LatencyResult>>,
    ping_tx: Sender<LatencyResult>,
    ping_pending: bool,
    next_ping: Option<Moment>,
    closed: Arc<AtomicBool>,
}

impl Default for IdentityPanel {
    fn default() -> Self {
        IdentityPanel::new()
    }
}

impl IdentityPanel {
    /// Loads the saved encrypted key, as `newIdentityPanel`.
    pub fn new() -> IdentityPanel {
        let (results_tx, results) = channel();
        let (ping_tx, ping_results) = channel();
        let (host, database) = storage::network_defaults();
        let mut p = IdentityPanel {
            fields: Default::default(),
            host,
            database,
            backup: Vec::new(),
            key: None,
            session: None,
            message: "Your key is your account. Email is optional.".to_string(),
            failed: false,
            busy: false,
            latency: Duration::ZERO,
            focus: None,
            results: Mutex::new(results),
            results_tx,
            ping_results: Mutex::new(ping_results),
            ping_tx,
            ping_pending: false,
            next_ping: None,
            closed: Arc::new(AtomicBool::new(false)),
        };
        p.fields[1] = storage::default_transfer();
        match storage::load_backup() {
            Ok(backup) => p.backup = backup,
            Err(e) => {
                p.message = e.to_string();
                p.failed = true;
            }
        }
        if !p.backup.is_empty() && document_account_id(&p.backup).is_empty() {
            p.message = "The saved key is damaged. Import a valid backup.".to_string();
            p.failed = true;
        }
        p
    }

    /// Closes the connection and forgets the unlocked key. A job still
    /// running sees `closed` and shuts any session it produces.
    pub fn close(&mut self) {
        self.closed.store(true, Ordering::SeqCst);
        if let Some(session) = self.session.take() {
            session.close();
        }
        self.key = None;
        self.fields[0].clear();
        if let Ok(r) = self.results.lock().unwrap().try_recv()
            && let Some(session) = r.session
        {
            session.close();
        }
    }

    /// Applies finished jobs, then keeps the latency probe going.
    pub fn poll(&mut self) {
        let result = self.results.lock().unwrap().try_recv().ok();
        if let Some(r) = result {
            self.busy = false;
            if let Some(err) = r.err {
                self.message = err.to_string();
                self.failed = true;
                self.poll_latency();
                return;
            }
            if let Some(backup) = r.backup {
                self.backup = backup;
            }
            if let Some(key) = r.key {
                if let Some(session) = self.session.take() {
                    session.close();
                }
                self.key = Some(key);
                self.fields[0].clear();
            }
            if let Some(session) = r.session {
                if let Some(old) = self.session.replace(session) {
                    old.close();
                }
                self.latency = Duration::ZERO;
                self.next_ping = None;
                self.ping_pending = false;
            }
            if let Some(email) = r.email {
                self.fields[2] = email;
            }
            if let Some(name) = r.display_name
                && self.fields[3].is_empty()
            {
                self.fields[3] = name;
            }
            self.message = r.message.to_string();
            self.failed = false;
        }
        self.poll_latency();
    }

    fn poll_latency(&mut self) {
        let result = self.ping_results.lock().unwrap().try_recv().ok();
        if let Some(result) = result
            && self
                .session
                .as_ref()
                .is_some_and(|s| Arc::ptr_eq(s, &result.session))
        {
            self.ping_pending = false;
            self.latency = result.latency.unwrap_or(Duration::ZERO);
        }
        let Some(session) = self.session.clone().filter(|s| s.is_active()) else {
            self.latency = Duration::ZERO;
            return;
        };
        let now = Moment::now();
        if self.ping_pending || self.next_ping.is_some_and(|next| now.is_before(next)) {
            return;
        }
        self.ping_pending = true;
        self.next_ping = Some(now.plus(Duration::from_secs(3)));
        let tx = self.ping_tx.clone();
        jobs::spawn(async move {
            let latency = session.ping(Duration::from_secs(2)).await;
            let _ = tx.send(LatencyResult { session, latency });
        });
    }

    fn start<F: Future<Output = IdentityResult> + MaybeSend + 'static>(&mut self, job: F) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.failed = false;
        self.message = "Working…".to_string();
        let tx = self.results_tx.clone();
        let closed = Arc::clone(&self.closed);
        jobs::spawn(async move {
            let result = job.await;
            if closed.load(Ordering::SeqCst)
                && let Some(session) = &result.session
            {
                session.close();
            }
            let _ = tx.send(result);
        });
    }

    pub fn enabled(&self, a: IdentityAction) -> bool {
        use IdentityAction::*;
        if a == Back {
            return true;
        }
        if self.busy {
            return false;
        }
        let connected = self.session.as_ref().is_some_and(|s| s.is_active());
        match a {
            Create => self.backup.is_empty() && !self.fields[0].is_empty(),
            Unlock => !self.backup.is_empty() && !self.fields[0].is_empty(),
            Import => !self.fields[0].is_empty(),
            Export => !self.backup.is_empty(),
            Connect | Lock => self.key.is_some(),
            Email | Name => self.key.is_some() && connected,
            Back => true,
        }
    }

    pub fn act(&mut self, a: IdentityAction) {
        use IdentityAction::*;
        if !self.enabled(a) {
            self.message = "Enter your key passphrase, or unlock your account first.".to_string();
            self.failed = true;
            return;
        }
        let fields = self.fields.clone();
        let backup = self.backup.clone();
        let key = self.key.clone();
        let session = self.session.clone();
        let (host, database) = (self.host.clone(), self.database.clone());
        match a {
            Create => self.start(async move {
                let k = match Key::generate() {
                    Ok(k) => k,
                    Err(e) => return IdentityResult::error(e),
                };
                let data = k.export(&fields[0]);
                let (data, err) = match data {
                    Ok(data) => {
                        let err = storage::save_backup(&data).err();
                        (Some(data), err)
                    }
                    Err(e) => (None, Some(e)),
                };
                IdentityResult {
                    backup: data,
                    key: Some(k),
                    err,
                    ..IdentityResult::message(
                        "Identity created. Export a backup, then connect when ready.",
                    )
                }
            }),
            Unlock => self.start(async move {
                match import(&backup, &fields[0]) {
                    Ok(k) => IdentityResult {
                        key: Some(k),
                        ..IdentityResult::message(
                            "Key unlocked. Connect to authorize your account.",
                        )
                    },
                    Err(e) => IdentityResult::error(e),
                }
            }),
            Import => self.start(async move {
                let data = match storage::read_transfer(&fields[1]).await {
                    Ok(data) => data,
                    Err(e) => return IdentityResult::error(e),
                };
                let (key, err) = match import(&data, &fields[0]) {
                    Ok(k) => (Some(k), storage::save_backup(&data).err()),
                    Err(e) => (None, Some(e)),
                };
                IdentityResult {
                    backup: Some(data),
                    key,
                    err,
                    ..IdentityResult::message("Identity imported. Your account ID is unchanged.")
                }
            }),
            Export => self.start(async move {
                IdentityResult {
                    err: storage::export_transfer(&backup, &fields[1]).err(),
                    ..IdentityResult::message(
                        "Encrypted key exported. Keep it and its passphrase safe.",
                    )
                }
            }),
            Connect => self.start(async move {
                let Some(key) = key else {
                    return IdentityResult::error(earth_two_identity::Error::new(
                        "account: a player key is required",
                    ));
                };
                // Go waits 20 s natively and 45 s in the browser for the
                // handshake; the connection itself outlives this job.
                let timeout = if cfg!(target_arch = "wasm32") {
                    Duration::from_secs(45)
                } else {
                    Duration::from_secs(20)
                };
                let s = match AccountSession::connect(&host, &database, &key, timeout).await {
                    Ok(s) => s,
                    Err(e) => return IdentityResult::error(e),
                };
                let d = match s.details() {
                    Ok(d) => d,
                    Err(e) => {
                        s.close();
                        return IdentityResult::error(e);
                    }
                };
                IdentityResult {
                    session: Some(Arc::new(s)),
                    email: Some(d.email),
                    display_name: Some(d.display_name),
                    ..IdentityResult::message("Account connected. Your key authorizes changes.")
                }
            }),
            Email => self.start(async move {
                let Some(session) = session else {
                    return IdentityResult::error(earth_two_identity::Error::new(
                        "account disconnected",
                    ));
                };
                IdentityResult {
                    err: session
                        .set_email(&fields[2], Duration::from_secs(30))
                        .await
                        .err(),
                    ..IdentityResult::message(
                        "Optional contact email saved. It cannot recover your key.",
                    )
                }
            }),
            Name => self.start(async move {
                let Some(session) = session else {
                    return IdentityResult::error(earth_two_identity::Error::new(
                        "account disconnected",
                    ));
                };
                IdentityResult {
                    err: session
                        .set_display_name(&fields[3], Duration::from_secs(30))
                        .await
                        .err(),
                    ..IdentityResult::message(
                        "Display name saved. Your signing key remains your identity.",
                    )
                }
            }),
            Lock => {
                if let Some(session) = self.session.take() {
                    session.close();
                }
                self.key = None;
                self.fields[0].clear();
                self.message = "Key locked. You can still export its encrypted backup.".to_string();
                self.failed = false;
            }
            Back => {}
        }
    }

    /// Types into the focused field, as `identityPanel.edit`: backspace
    /// removes a character, the modifier with A clears, with V pastes, and
    /// printable characters append within the field's byte limit.
    pub fn edit(&mut self, focus: usize, input: &TextInput) {
        if self.busy || focus >= IDENTITY_FIELDS || (focus == 1 && !CAN_EDIT_TRANSFER) {
            return;
        }
        let value = &mut self.fields[focus];
        if input.backspace {
            value.pop();
        }
        if input.modifier && input.select_all {
            value.clear();
        }
        let limit = FIELD_LIMITS[focus];
        let pasted = storage::paste_text(input.modifier && input.paste);
        for ch in pasted.chars() {
            if ch >= ' ' && ch != '\x7f' && value.len() + ch.len_utf8() <= limit {
                value.push(ch);
            }
        }
        for &ch in &input.chars {
            if !input.modifier && ch >= ' ' && ch != '\x7f' && value.len() + ch.len_utf8() <= limit
            {
                value.push(ch);
            }
        }
    }

    /// The saved account's ID, or the Go form's placeholder.
    pub fn account_line(&self) -> String {
        let mut id = document_account_id(&self.backup);
        if id.is_empty() {
            id = "No identity saved".to_string();
        }
        let state = if self.session.as_ref().is_some_and(|s| s.is_active()) {
            "Connected"
        } else if self.key.is_some() {
            "Unlocked"
        } else {
            "Locked"
        };
        format!("{state} · {id}")
    }

    /// A field's displayed text: the passphrase masked, empty as a dash.
    pub fn field_text(&self, i: usize) -> String {
        let value = &self.fields[i];
        if i == 0 {
            return "•".repeat(value.chars().count().min(32));
        }
        if value.is_empty() {
            "—".to_string()
        } else {
            value.clone()
        }
    }

    /// The connection HUD's label and RGBA colour, as `drawConnectionHUD`.
    pub fn connection_label(&self) -> (String, [u8; 4]) {
        if self.session.as_ref().is_some_and(|s| s.is_active()) {
            let mut label = "Connected".to_string();
            if self.latency > Duration::ZERO {
                let ms = (self.latency.as_secs_f64() * 1000.0).round() as u64;
                label.push_str(&format!(" · {ms} ms"));
            }
            (label, [161, 208, 164, 255])
        } else {
            ("Disconnected".to_string(), [226, 151, 129, 255])
        }
    }

    /// Whether the browser bridge should capture paste events: a text field
    /// is focused and no job is running.
    pub fn wants_clipboard(&self) -> bool {
        self.focus.is_some() && !self.busy
    }
}

/// Keeps the identity form's jobs and latency probe running each frame.
pub struct IdentityPlugin;

impl Plugin for IdentityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<IdentityPanel>()
            .add_systems(Update, (poll_identity, edit_from_input).chain());
    }
}

fn poll_identity(mut panel: ResMut<IdentityPanel>) {
    panel.poll();
    storage::clipboard_focus(panel.wants_clipboard());
}

/// Feeds keyboard text into the focused field. The menus set `focus`.
fn edit_from_input(
    mut panel: ResMut<IdentityPanel>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut keyboard: Option<MessageReader<bevy::input::keyboard::KeyboardInput>>,
) {
    let Some(focus) = panel.focus else {
        return;
    };
    let (Some(keys), Some(keyboard)) = (keys, keyboard.as_mut()) else {
        return;
    };
    let modifier = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    let mut input = TextInput {
        backspace: keys.just_pressed(KeyCode::Backspace),
        modifier,
        select_all: keys.just_pressed(KeyCode::KeyA),
        paste: keys.just_pressed(KeyCode::KeyV),
        chars: Vec::new(),
    };
    for event in keyboard.read() {
        if event.state.is_pressed()
            && let bevy::input::keyboard::Key::Character(text) = &event.logical_key
        {
            input.chars.extend(text.chars());
        }
    }
    panel.edit(focus, &input);
}
