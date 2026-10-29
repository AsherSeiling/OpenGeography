//! The user's API keys in the operating system's keychain (macOS Keychain, Windows Credential Manager, the Secret Service
//! on Linux). A key can be stored, checked for and deleted by the UI; it is never sent back to the UI, written to a file, logged
//! or put in an error message. Only the shell reads it, to pass it to the engine for one request.

const SERVICE: &str = "GIS Studio";

fn entry(name: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, name).map_err(|e| format!("The system keychain is not available: {e}"))
}

pub fn set(name: &str, value: &str) -> Result<(), String> {
    let v = value.trim();
    if v.is_empty() {
        return Err("Enter the key first".into());
    }
    entry(name)?.set_password(v).map_err(|e| format!("The key could not be saved in the system keychain: {e}"))
}

/// The key, if one is stored. Errors (a locked or missing keychain) read as "no key".
pub fn get(name: &str) -> Option<String> {
    entry(name).ok()?.get_password().ok().filter(|k| !k.trim().is_empty())
}

pub fn has(name: &str) -> bool {
    get(name).is_some()
}

pub fn delete(name: &str) -> Result<(), String> {
    match entry(name)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("The key could not be removed from the system keychain: {e}")),
    }
}

/// Name of the Census Bureau key.
pub const CENSUS: &str = "census-api-key";
