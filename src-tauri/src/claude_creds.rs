//! Reading and writing the Claude Code CLI's own login.
//!
//! Claude Code keeps the active login in two places:
//! * `<config dir>/.credentials.json` — key `claudeAiOauth` (access/refresh token, plan) next to
//!   unrelated keys such as `mcpOAuth` that must never be touched;
//! * `~/.claude.json` — key `oauthAccount` (email, organization, ...).
//!
//! Switching accounts replaces exactly those two values and nothing else.

use serde_json::{Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct CliLogin {
    /// The `claudeAiOauth` object.
    pub oauth: Value,
    /// The `oauthAccount` object, when present.
    pub oauth_account: Option<Value>,
}

impl CliLogin {
    pub fn email(&self) -> Option<String> {
        self.oauth_account
            .as_ref()
            .and_then(|a| a.get("emailAddress"))
            .and_then(Value::as_str)
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
    }

    pub fn access_token(&self) -> Option<&str> {
        self.oauth.get("accessToken").and_then(Value::as_str)
    }


    pub fn expires_at_ms(&self) -> Option<i64> {
        self.oauth.get("expiresAt").and_then(Value::as_i64)
    }

    pub fn subscription(&self) -> String {
        self.oauth
            .get("subscriptionType")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    pub fn rate_limit_tier(&self) -> String {
        self.oauth
            .get("rateLimitTier")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }
}

pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    dirs::home_dir().unwrap_or_default().join(".claude")
}

pub fn credentials_path() -> PathBuf {
    config_dir().join(".credentials.json")
}

/// With a custom CLAUDE_CONFIG_DIR the global config lives inside it, otherwise next to `~/.claude`.
pub fn claude_json_path() -> PathBuf {
    if std::env::var_os("CLAUDE_CONFIG_DIR").is_some_and(|v| !v.is_empty()) {
        config_dir().join(".claude.json")
    } else {
        dirs::home_dir().unwrap_or_default().join(".claude.json")
    }
}

fn read_json(path: &Path) -> io::Result<Option<Value>> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {e}", path.display()))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Write via a temp file in the same directory, then rename over the original.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.flipon-tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file")
    ));
    fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

pub fn read_cli_login() -> Result<Option<CliLogin>, String> {
    #[cfg(target_os = "macos")]
    {
        // Claude Code stores its login in the macOS Keychain there, which FlipOn does not read yet.
        return Err("Reading the Claude Code login from the macOS Keychain is not supported yet.".into());
    }
    #[allow(unreachable_code)]
    {
        let creds = read_json(&credentials_path()).map_err(|e| e.to_string())?;
        let Some(oauth) = creds.as_ref().and_then(|c| c.get("claudeAiOauth")).cloned() else {
            return Ok(None);
        };
        let oauth_account = read_json(&claude_json_path())
            .map_err(|e| e.to_string())?
            .and_then(|c| c.get("oauthAccount").cloned());
        Ok(Some(CliLogin { oauth, oauth_account }))
    }
}

pub fn write_cli_login(login: &CliLogin) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let _ = login;
        return Err("Writing the Claude Code login to the macOS Keychain is not supported yet.".into());
    }
    #[allow(unreachable_code)]
    {
        // credentials: keep every other key (mcpOAuth, ...), replace only claudeAiOauth
        let path = credentials_path();
        let mut creds = match read_json(&path).map_err(|e| e.to_string())? {
            Some(Value::Object(map)) => map,
            _ => Map::new(),
        };
        creds.insert("claudeAiOauth".into(), login.oauth.clone());
        let bytes = serde_json::to_vec(&Value::Object(creds)).map_err(|e| e.to_string())?;
        write_atomic(&path, &bytes).map_err(|e| format!("Could not write {}: {e}", path.display()))?;

        // identity: replace only oauthAccount in ~/.claude.json
        if let Some(account) = &login.oauth_account {
            let path = claude_json_path();
            if let Some(Value::Object(mut map)) = read_json(&path).map_err(|e| e.to_string())? {
                map.insert("oauthAccount".into(), account.clone());
                let bytes = serde_json::to_vec_pretty(&Value::Object(map)).map_err(|e| e.to_string())?;
                write_atomic(&path, &bytes).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn switching_replaces_only_the_login_and_keeps_everything_else() {
        let dir = std::env::temp_dir().join(format!("flipon-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        // SAFETY: this is the only test that touches CLAUDE_CONFIG_DIR.
        unsafe { std::env::set_var("CLAUDE_CONFIG_DIR", &dir) };

        fs::write(
            dir.join(".credentials.json"),
            json!({"claudeAiOauth": {"accessToken": "old"}, "mcpOAuth": {"server": {"token": "keep"}}}).to_string(),
        )
        .unwrap();
        fs::write(
            dir.join(".claude.json"),
            json!({"oauthAccount": {"emailAddress": "old@x.com"}, "numStartups": 5}).to_string(),
        )
        .unwrap();

        let new = CliLogin {
            oauth: json!({"accessToken": "new", "subscriptionType": "max"}),
            oauth_account: Some(json!({"emailAddress": "New@X.com"})),
        };
        write_cli_login(&new).unwrap();

        let creds: Value = serde_json::from_str(&fs::read_to_string(dir.join(".credentials.json")).unwrap()).unwrap();
        assert_eq!(creds["claudeAiOauth"]["accessToken"], "new");
        assert_eq!(creds["mcpOAuth"]["server"]["token"], "keep");
        let global: Value = serde_json::from_str(&fs::read_to_string(dir.join(".claude.json")).unwrap()).unwrap();
        assert_eq!(global["oauthAccount"]["emailAddress"], "New@X.com");
        assert_eq!(global["numStartups"], 5);

        let read = read_cli_login().unwrap().unwrap();
        assert_eq!(read.email().as_deref(), Some("new@x.com"));
        assert_eq!(read.subscription(), "max");
        assert!(!dir.join(".credentials.json.flipon-tmp").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
