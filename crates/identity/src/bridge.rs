//! The browser transport: the existing web networking adapter
//! (`web/spacetime/bridge.ts`, exposed as `globalThis.EarthTwoAccount`), as
//! Go's internals/account/session_js.go uses it. The adapter does networking
//! only; this crate stores backups and signs every account action.

use std::time::Duration;

use js_sys::{Function, Promise, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::{
    Error,
    proof::Context,
    session::{Details, Transport},
};

pub struct BridgeTransport {
    handle: f64,
    context: Context,
}

fn bridge() -> Result<JsValue, Error> {
    let b = Reflect::get(&js_sys::global(), &"EarthTwoAccount".into()).ok();
    match b {
        Some(b) if !b.is_undefined() && !b.is_null() => Ok(b),
        _ => Err(Error::new(
            "account: browser networking bridge is not loaded",
        )),
    }
}

fn call(method: &str, args: &[JsValue]) -> Result<JsValue, Error> {
    let b = bridge()?;
    let f: Function = Reflect::get(&b, &method.into())
        .ok()
        .and_then(|f| f.dyn_into().ok())
        .ok_or_else(|| Error::new("account: browser networking bridge is incomplete"))?;
    let array = js_sys::Array::new();
    for a in args {
        array.push(a);
    }
    Reflect::apply(&f, &b, &array).map_err(js_error)
}

/// A rejected promise's message, as Go's `promise` helper reads it.
fn js_error(value: JsValue) -> Error {
    if let Some(text) = value.as_string() {
        return Error::new(text);
    }
    match Reflect::get(&value, &"message".into()) {
        Ok(m) if !m.is_undefined() => Error::new(m.as_string().unwrap_or_default()),
        _ => Error::new("account: browser operation failed"),
    }
}

async fn promised(value: JsValue) -> Result<JsValue, Error> {
    let promise: Promise = value
        .dyn_into()
        .map_err(|_| Error::new("account: browser operation failed"))?;
    JsFuture::from(promise).await.map_err(js_error)
}

/// A JS timer as a future, for waiting between cache checks and for the
/// adapter's own timeouts to settle.
pub async fn sleep(d: Duration) {
    let millis = d.as_millis() as f64;
    let promise = Promise::new(&mut |resolve, _| {
        let set_timeout: Function = Reflect::get(&js_sys::global(), &"setTimeout".into())
            .ok()
            .and_then(|f| f.dyn_into().ok())
            .expect("setTimeout");
        let _ = set_timeout.call2(&JsValue::UNDEFINED, &resolve, &millis.into());
    });
    let _ = JsFuture::from(promise).await;
}

fn raw_id(text: &str, target: &mut [u8]) -> Result<(), Error> {
    let decoded = hex::decode(text).map_err(|_| Error::new("account: invalid network identity"))?;
    if decoded.len() != target.len() {
        return Err(Error::new("account: invalid network identity"));
    }
    for (i, b) in decoded.iter().enumerate() {
        target[target.len() - 1 - i] = *b;
    }
    Ok(())
}

fn field(value: &JsValue, name: &str) -> JsValue {
    Reflect::get(value, &name.into()).unwrap_or(JsValue::UNDEFINED)
}

fn string_field(value: &JsValue, name: &str) -> String {
    let v = field(value, name);
    v.as_string()
        .or_else(|| v.as_f64().map(|n| format!("{n}")))
        .unwrap_or_default()
}

impl Transport for BridgeTransport {
    async fn connect(
        host: &str,
        database: &str,
        account_id: &str,
        _timeout: Duration,
    ) -> Result<BridgeTransport, Error> {
        // The adapter applies its own 10 s lookup and 15 s connect and
        // subscribe limits; Go's 45 s outer wait only covers the promise.
        let v = promised(call(
            "connect",
            &[host.into(), database.into(), account_id.into()],
        )?)
        .await?;
        let mut t = BridgeTransport {
            handle: field(&v, "handle").as_f64().unwrap_or(0.0),
            context: Context::default(),
        };
        let ids = raw_id(&string_field(&v, "database"), &mut t.context.database)
            .and_then(|_| raw_id(&string_field(&v, "sender"), &mut t.context.sender))
            .and_then(|_| raw_id(&string_field(&v, "connection"), &mut t.context.connection));
        if let Err(e) = ids {
            t.close();
            return Err(e);
        }
        Ok(t)
    }

    fn context(&self) -> Context {
        self.context
    }

    fn is_active(&self) -> bool {
        call("active", &[self.handle.into()])
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    fn close(&self) {
        let _ = call("close", &[self.handle.into()]);
    }

    fn revision(&self) -> u64 {
        call("revision", &[self.handle.into()])
            .ok()
            .and_then(|v| v.as_string())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }

    fn details(&self) -> Option<Details> {
        let v = call("details", &[self.handle.into()]).ok()?;
        if v.is_null() || v.is_undefined() {
            return None;
        }
        let public = field(&v, "publicKey");
        let public_key = if public.is_instance_of::<Uint8Array>() {
            Uint8Array::from(public).to_vec()
        } else {
            Vec::new()
        };
        Some(Details {
            id: string_field(&v, "id"),
            public_key,
            revision: string_field(&v, "revision").parse().ok()?,
            email: string_field(&v, "email"),
            display_name: string_field(&v, "displayName"),
        })
    }

    async fn call(
        &self,
        action: &str,
        text: &str,
        proof: Vec<u8>,
        _timeout: Duration,
    ) -> Result<(), Error> {
        let bytes = Uint8Array::from(proof.as_slice());
        promised(call(
            "mutate",
            &[self.handle.into(), action.into(), text.into(), bytes.into()],
        )?)
        .await
        .map(|_| ())
    }

    async fn ping(&self, nonce: u64, _timeout: Duration) -> Result<u64, Error> {
        // The adapter takes and returns the nonce as decimal text.
        let value = promised(call(
            "ping",
            &[self.handle.into(), nonce.to_string().into()],
        )?)
        .await?;
        value
            .as_string()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| Error::new("invalid latency probe response"))
    }

    async fn next_event(&self, timeout: Duration) -> Result<(), Error> {
        // The adapter resolves a mutation after its reducer committed, so the
        // cache is normally current already; otherwise poll briefly.
        if timeout.is_zero() {
            return Err(Error::new("context deadline exceeded"));
        }
        sleep(timeout.min(Duration::from_millis(50))).await;
        if !self.is_active() {
            return Err(Error::new("account disconnected"));
        }
        Ok(())
    }
}

/// Clipboard capture for the identity form's text fields; the adapter
/// listens for paste events while a field is focused.
pub fn capture_paste(active: bool) {
    let _ = call("capturePaste", &[active.into()]);
}

pub fn take_paste() -> String {
    call("takePaste", &[])
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default()
}
