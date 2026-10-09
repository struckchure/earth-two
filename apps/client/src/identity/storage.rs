//! Where the encrypted active key lives, and how transfers move: files on
//! desktop (game/identity_storage_native.go), browser storage plus a file
//! chooser and download in the browser (game/identity_storage_js.go). The
//! paths and storage keys are the Go client's, so an installed identity
//! keeps working across both clients.

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
#[cfg(target_arch = "wasm32")]
pub use web::*;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::path::{Path, PathBuf};

    use earth_two_identity::{
        Error,
        backup::document_public_key,
        document_account_id,
        file::{read_backup, write_atomically, write_backup, write_public_key},
    };

    /// `EARTH_TWO_IDENTITY_PATH`, or `~/earth-two/pk`.
    pub fn profile_path() -> Result<PathBuf, Error> {
        if let Ok(path) = std::env::var("EARTH_TWO_IDENTITY_PATH")
            && !path.is_empty()
        {
            return Ok(PathBuf::from(path));
        }
        let home = std::env::home_dir().ok_or_else(|| Error::new("$HOME is not defined"))?;
        Ok(home.join("earth-two").join("pk"))
    }

    pub fn load_backup() -> Result<Vec<u8>, Error> {
        let path = profile_path()?;
        match std::fs::metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e.into()),
            Ok(_) => read_backup(&path),
        }
    }

    pub fn save_backup(data: &[u8]) -> Result<(), Error> {
        let path = profile_path()?;
        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        std::fs::create_dir_all(&dir)?;
        // Preserve an existing account before importing another identity.
        if let Ok(old) = read_backup(&path) {
            let id = document_account_id(&old);
            if !id.is_empty() {
                let archive = dir
                    .join("identities")
                    .join(format!("{id}.earth-two-key.json"));
                if !archive.exists() {
                    write_backup(&archive, &old)?;
                }
            }
        }
        write_atomically(&dir, ".identity-", &path, data, 0o600)?;
        write_public_key(&dir.join("pub"), &document_public_key(data)?)
    }

    pub fn default_transfer() -> String {
        match profile_path() {
            Ok(path) => path
                .parent()
                .unwrap_or(Path::new(""))
                .join("backup.earth-two-key.json")
                .to_string_lossy()
                .into_owned(),
            Err(_) => "backup.earth-two-key.json".to_string(),
        }
    }

    pub async fn read_transfer(path: &str) -> Result<Vec<u8>, Error> {
        read_backup(Path::new(path))
    }

    pub fn export_transfer(data: &[u8], path: &str) -> Result<(), Error> {
        write_backup(Path::new(path), data)
    }

    pub fn network_defaults() -> (String, String) {
        earth_two_identity::config::network_defaults()
    }

    /// Desktop input supplies text through Bevy's system clipboard resource.
    /// Only the browser needs a separately captured paste event.
    pub fn paste_text(_pressed: bool) -> String {
        String::new()
    }

    pub fn clipboard_focus(_active: bool) {}
}

#[cfg(target_arch = "wasm32")]
mod web {
    use earth_two_identity::{Error, backup::document_public_key, bridge, document_account_id};
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, HtmlInputElement, Storage, Url};

    const ACTIVE: &str = "earth-two/identity/active";

    fn storage() -> Option<Storage> {
        web_sys::window()?.local_storage().ok().flatten()
    }

    pub fn load_backup() -> Result<Vec<u8>, Error> {
        let storage = storage().ok_or_else(|| Error::new("browser storage is unavailable"))?;
        match storage.get_item(ACTIVE) {
            Ok(Some(text)) => Ok(text.into_bytes()),
            Ok(None) => Ok(Vec::new()),
            Err(_) => Err(Error::new("browser storage is unavailable")),
        }
    }

    pub fn save_backup(data: &[u8]) -> Result<(), Error> {
        let failed = || Error::new("could not save encrypted identity in browser storage");
        let storage = storage().ok_or_else(failed)?;
        if let Ok(Some(old)) = storage.get_item(ACTIVE) {
            let id = document_account_id(old.as_bytes());
            if !id.is_empty() {
                storage
                    .set_item(&format!("earth-two/identity/saved/{id}"), &old)
                    .map_err(|_| failed())?;
            }
        }
        let text = String::from_utf8_lossy(data);
        storage.set_item(ACTIVE, &text).map_err(|_| failed())?;
        let public = document_public_key(data)?;
        storage
            .set_item("earth-two/identity/pub", &hex::encode(public))
            .map_err(|_| failed())
    }

    pub fn default_transfer() -> String {
        "Import opens a file chooser; export downloads your key".to_string()
    }

    /// Opens a file chooser and reads the picked backup.
    pub async fn read_transfer(_path: &str) -> Result<Vec<u8>, Error> {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or_else(|| Error::new("could not read key backup"))?;
        let input: HtmlInputElement = document
            .create_element("input")
            .ok()
            .and_then(|e| e.dyn_into().ok())
            .ok_or_else(|| Error::new("could not read key backup"))?;
        input.set_type("file");
        input.set_accept(".json,application/json");
        let _ = input.style().set_property("display", "none");
        if let Some(body) = document.body() {
            let _ = body.append_child(&input);
        }
        let picked = js_sys::Promise::new(&mut |resolve, _| {
            let change = {
                let input = input.clone();
                let resolve = resolve.clone();
                Closure::<dyn FnMut()>::new(move || {
                    let file = input.files().and_then(|list| list.get(0));
                    let _ = resolve.call1(
                        &JsValue::UNDEFINED,
                        &file.map(JsValue::from).unwrap_or(JsValue::NULL),
                    );
                })
            };
            let cancel = Closure::<dyn FnMut()>::new(move || {
                let _ = resolve.call1(&JsValue::UNDEFINED, &JsValue::NULL);
            });
            input.set_onchange(Some(change.as_ref().unchecked_ref()));
            input.set_oncancel(Some(cancel.as_ref().unchecked_ref()));
            change.forget();
            cancel.forget();
        });
        input.click();
        let file = JsFuture::from(picked).await.unwrap_or(JsValue::NULL);
        input.set_onchange(None);
        input.set_oncancel(None);
        input.remove();
        if file.is_null() {
            return Err(Error::new("key import cancelled"));
        }
        let file: web_sys::File = file
            .dyn_into()
            .map_err(|_| Error::new("could not read key backup"))?;
        if file.size() > 4096.0 {
            return Err(Error::new("key backup is too large"));
        }
        let text = JsFuture::from(file.text())
            .await
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        if text.is_empty() {
            return Err(Error::new("could not read key backup"));
        }
        Ok(text.into_bytes())
    }

    /// Downloads the encrypted key as `<account id>.earth-two-key.json`.
    pub fn export_transfer(data: &[u8], _path: &str) -> Result<(), Error> {
        let failed = || Error::new("could not export key backup");
        let bytes = js_sys::Uint8Array::from(data);
        let parts = js_sys::Array::new();
        parts.push(&bytes);
        let options = BlobPropertyBag::new();
        options.set_type("application/json");
        let blob =
            Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(|_| failed())?;
        let url = Url::create_object_url_with_blob(&blob).map_err(|_| failed())?;
        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or_else(failed)?;
        let anchor: HtmlAnchorElement = document
            .create_element("a")
            .ok()
            .and_then(|e| e.dyn_into().ok())
            .ok_or_else(failed)?;
        anchor.set_href(&url);
        anchor.set_download(&format!("{}.earth-two-key.json", document_account_id(data)));
        anchor.click();
        let revoke = Closure::<dyn FnMut()>::new(move || {
            let _ = Url::revoke_object_url(&url);
        });
        if let Some(window) = web_sys::window() {
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                revoke.as_ref().unchecked_ref(),
                1000,
            );
        }
        revoke.forget();
        Ok(())
    }

    pub fn paste_text(_pressed: bool) -> String {
        bridge::take_paste()
    }

    pub fn clipboard_focus(active: bool) {
        bridge::capture_paste(active);
    }

    /// `?server=` and `?database=` on the page URL override the defaults.
    pub fn network_defaults() -> (String, String) {
        let (mut host, mut db) = (
            earth_two_identity::config::DEFAULT_HOST.to_string(),
            earth_two_identity::config::DEFAULT_DATABASE.to_string(),
        );
        let search = web_sys::window()
            .and_then(|w| w.location().search().ok())
            .unwrap_or_default();
        if let Ok(params) = web_sys::UrlSearchParams::new_with_str(&search) {
            if let Some(v) = params.get("server") {
                host = v;
            }
            if let Some(v) = params.get("database") {
                db = v;
            }
        }
        (host, db)
    }
}
