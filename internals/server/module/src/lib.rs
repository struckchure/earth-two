use spacetimedb::{ConnectionId, Identity, ReducerContext, SpacetimeType, Table, ViewContext};

mod ota;
mod proof;

// Read-only round-trip probe; it never advances a player's authorization sequence.
#[spacetimedb::procedure]
pub fn connection_ping(_ctx: &mut spacetimedb::ProcedureContext, nonce: u64) -> u64 {
    nonce
}

// Public keys and revisions permit clients to prepare an authorization before
// linking. Personal metadata and transport bindings stay private.
#[spacetimedb::table(accessor = account, public)]
pub struct Account {
    #[primary_key]
    pub id: String,
    pub public_key: Vec<u8>,
    pub revision: u64,
    pub display_name: String,
}

#[spacetimedb::table(accessor = account_email)]
pub struct AccountEmail {
    #[primary_key]
    pub account_id: String,
    pub email: String,
}

#[spacetimedb::table(accessor = account_binding)]
pub struct AccountBinding {
    #[primary_key]
    pub sender: Identity,
    pub account_id: String,
}

#[spacetimedb::table(accessor = account_connection)]
pub struct AccountConnection {
    #[primary_key]
    pub connection: ConnectionId,
    #[index(btree)]
    pub sender: Identity,
}

#[spacetimedb::reducer(client_connected)]
pub fn connected(ctx: &ReducerContext) {
    if let Some(connection) = ctx.connection_id() {
        ctx.db.account_connection().insert(AccountConnection {
            connection,
            sender: ctx.sender(),
        });
    }
}

#[spacetimedb::reducer(client_disconnected)]
pub fn disconnected(ctx: &ReducerContext) {
    if let Some(connection) = ctx.connection_id() {
        ctx.db.account_connection().connection().delete(connection);
        // Keep a binding while another connection with the same transport
        // identity is still alive. Offline tokens must prove key ownership anew.
        if ctx
            .db
            .account_connection()
            .sender()
            .filter(ctx.sender())
            .next()
            .is_none()
        {
            ctx.db.account_binding().sender().delete(ctx.sender());
        }
    }
}

#[derive(SpacetimeType)]
pub struct AccountDetails {
    pub id: String,
    pub public_key: Vec<u8>,
    pub revision: u64,
    pub email: String,
    pub display_name: String,
}

fn verify(
    ctx: &ReducerContext,
    action: &str,
    payload: &[u8],
    bytes: &[u8],
) -> Result<proof::Authorization, String> {
    let connection = ctx
        .connection_id()
        .ok_or("a live player connection is required")?;
    proof::verify(
        ctx.database_identity().to_byte_array(),
        ctx.sender().to_byte_array(),
        connection.as_le_byte_array(),
        action,
        payload,
        bytes,
        ctx.timestamp.to_micros_since_unix_epoch(),
    )
}

// Every future player mutation must pass through this guard. The revision
// update and its game effects commit or roll back together in one reducer.
pub fn authorize_account(
    ctx: &ReducerContext,
    action: &str,
    payload: &[u8],
    bytes: &[u8],
) -> Result<Account, String> {
    let auth = verify(ctx, action, payload, bytes)?;
    let mut account = checked_account(ctx, &auth)?;
    let binding = ctx
        .db
        .account_binding()
        .sender()
        .find(ctx.sender())
        .ok_or("link this connection to the account first")?;
    if binding.account_id != account.id {
        return Err("connection belongs to another account".into());
    }
    account.revision = account
        .revision
        .checked_add(1)
        .ok_or("authorization sequence exhausted")?;
    Ok(ctx.db.account().id().update(account))
}

fn checked_account(ctx: &ReducerContext, auth: &proof::Authorization) -> Result<Account, String> {
    let account = ctx
        .db
        .account()
        .id()
        .find(&auth.account_id)
        .ok_or("account not found")?;
    if account.public_key != auth.public_key || account.revision != auth.sequence {
        return Err("stale authorization sequence".into());
    }
    Ok(account)
}

#[spacetimedb::reducer]
pub fn register_account(ctx: &ReducerContext, proof: Vec<u8>) -> Result<(), String> {
    let auth = verify(ctx, "account.register", &[], &proof)?;
    if auth.sequence != 0 {
        return Err("new accounts require sequence zero".into());
    }
    if ctx.db.account().id().find(&auth.account_id).is_some() {
        return Err("account already exists; link it instead".into());
    }
    if ctx
        .db
        .account_binding()
        .sender()
        .find(ctx.sender())
        .is_some()
    {
        return Err("connection is already linked".into());
    }
    ctx.db.account().insert(Account {
        id: auth.account_id.clone(),
        public_key: auth.public_key,
        revision: 1,
        display_name: "Player".into(),
    });
    ctx.db.account_binding().insert(AccountBinding {
        sender: ctx.sender(),
        account_id: auth.account_id.clone(),
    });
    log::info!("account registered: {}", auth.account_id);
    Ok(())
}

#[spacetimedb::reducer]
pub fn link_account(ctx: &ReducerContext, proof: Vec<u8>) -> Result<(), String> {
    let auth = verify(ctx, "account.link", &[], &proof)?;
    let mut account = checked_account(ctx, &auth)?;
    if let Some(binding) = ctx.db.account_binding().sender().find(ctx.sender()) {
        if binding.account_id != account.id {
            return Err("connection belongs to another account".into());
        }
    } else {
        ctx.db.account_binding().insert(AccountBinding {
            sender: ctx.sender(),
            account_id: account.id.clone(),
        });
    }
    account.revision = account
        .revision
        .checked_add(1)
        .ok_or("authorization sequence exhausted")?;
    ctx.db.account().id().update(account);
    Ok(())
}

#[spacetimedb::reducer]
pub fn set_account_email(
    ctx: &ReducerContext,
    email: String,
    proof: Vec<u8>,
) -> Result<(), String> {
    // Empty removes the address. Email is unverified contact metadata and never
    // authorizes login, recovery, key replacement, or any other account action.
    if !email.is_empty() && !valid_email(&email) {
        return Err("invalid email address".into());
    }
    let account = authorize_account(ctx, "account.set_email", email.as_bytes(), &proof)?;
    if email.is_empty() {
        ctx.db.account_email().account_id().delete(&account.id);
    } else if ctx
        .db
        .account_email()
        .account_id()
        .find(&account.id)
        .is_some()
    {
        ctx.db.account_email().account_id().update(AccountEmail {
            account_id: account.id,
            email,
        });
    } else {
        ctx.db.account_email().insert(AccountEmail {
            account_id: account.id,
            email,
        });
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn set_account_display_name(
    ctx: &ReducerContext,
    display_name: String,
    proof: Vec<u8>,
) -> Result<(), String> {
    // Names are labels, never ownership identifiers. Duplicates, Unicode,
    // spaces and punctuation are allowed; bound length and invisible controls.
    if display_name.chars().count() > 128 || display_name.chars().any(char::is_control) {
        return Err("display name must be at most 128 printable characters".into());
    }
    let mut account = authorize_account(
        ctx,
        "account.set_display_name",
        display_name.as_bytes(),
        &proof,
    )?;
    account.display_name = display_name;
    ctx.db.account().id().update(account);
    Ok(())
}

fn valid_email(email: &str) -> bool {
    if email.len() > 254
        || !email.is_ascii()
        || email
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
    {
        return false;
    }
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && local.len() <= 64
        && !domain.contains('@')
        && domain.contains('.')
        && !local.starts_with('.')
        && !local.ends_with('.')
        && !local.contains("..")
        && local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&b))
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

#[spacetimedb::view(accessor = my_account, public)]
pub fn my_account(ctx: &ViewContext) -> Vec<AccountDetails> {
    let Some(binding) = ctx.db.account_binding().sender().find(ctx.sender()) else {
        return vec![];
    };
    let Some(account) = ctx.db.account().id().find(&binding.account_id) else {
        return vec![];
    };
    let email = ctx
        .db
        .account_email()
        .account_id()
        .find(&account.id)
        .map(|e| e.email)
        .unwrap_or_default();
    vec![AccountDetails {
        id: account.id,
        public_key: account.public_key,
        revision: account.revision,
        email,
        display_name: account.display_name,
    }]
}
