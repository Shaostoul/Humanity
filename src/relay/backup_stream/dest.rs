//! Where the continuous backup sends its copies: a folder (another disk, a mounted share), an
//! S3-compatible bucket (any provider or a self-hosted one such as Garage or MinIO), or another
//! HumanityOS server that has agreed to keep this one's backups (`host.rs`). All three are the
//! same four operations on named objects, each object written whole or not at all.
//!
//! The configuration holds the only secret in this feature (an S3 secret access key). It lives
//! in `backup-destination.secrets.json` beside the database, is never sent back to a client
//! (`DestinationConfig::view`), never logged (`Debug` is written by hand), and never placed in a
//! URL: S3 requests are signed in a header (`s3.rs`), and a HumanityOS server is reached with
//! this server's own signature, also in a header.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::names::Listed;

/// Where to send the copies, as saved.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DestinationConfig {
    /// A folder on this machine: another disk, or a share mounted from another machine.
    Folder { path: String },
    /// An S3-compatible bucket.
    S3 {
        /// e.g. https://s3.eu-central-1.amazonaws.com, https://<account>.r2.cloudflarestorage.com,
        /// http://192.168.1.20:3900 (a Garage or MinIO of your own).
        endpoint: String,
        #[serde(default = "default_region")]
        region: String,
        bucket: String,
        /// A folder inside the bucket; empty for the bucket's root.
        #[serde(default)]
        prefix: String,
        access_key_id: String,
        #[serde(default)]
        secret_access_key: String,
        /// Bucket in the path (https://host/bucket/key), which every S3-compatible server
        /// accepts; false puts it in the host name (https://bucket.host/key).
        #[serde(default = "yes")]
        path_style: bool,
    },
    /// Another HumanityOS server that keeps this one's backups (its Backups panel lists this
    /// server's key under "Keep other servers' backups here").
    Server { url: String },
}

fn default_region() -> String {
    "us-east-1".into()
}
fn yes() -> bool {
    true
}

impl std::fmt::Debug for DestinationConfig {
    // By hand, so a secret can never reach a log through `{:?}`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.summary())
    }
}

impl DestinationConfig {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Folder { .. } => "folder",
            Self::S3 { .. } => "s3",
            Self::Server { .. } => "server",
        }
    }

    /// One line for people and logs: where, never how to get in.
    pub fn summary(&self) -> String {
        match self {
            Self::Folder { path } => format!("the folder {path}"),
            Self::S3 { endpoint, bucket, prefix, .. } => {
                if prefix.is_empty() {
                    format!("the bucket {bucket} at {endpoint}")
                } else {
                    format!("{prefix}/ in the bucket {bucket} at {endpoint}")
                }
            }
            Self::Server { url } => format!("the HumanityOS server {url}"),
        }
    }

    /// What a client is shown: every field except the secret, which is only said to be set.
    pub fn view(&self) -> serde_json::Value {
        match self {
            Self::Folder { path } => serde_json::json!({ "kind": "folder", "path": path, "summary": self.summary() }),
            Self::S3 { endpoint, region, bucket, prefix, access_key_id, secret_access_key, path_style } => serde_json::json!({
                "kind": "s3",
                "endpoint": endpoint,
                "region": region,
                "bucket": bucket,
                "prefix": prefix,
                "access_key_hint": mask(access_key_id),
                "secret_set": !secret_access_key.is_empty(),
                "path_style": path_style,
                "summary": self.summary(),
            }),
            Self::Server { url } => serde_json::json!({ "kind": "server", "url": url, "summary": self.summary() }),
        }
    }

    /// An edit from a client: the S3 key id and secret a client left empty keep the saved ones
    /// (a client is never sent them, so it cannot send them back).
    pub fn merged_with_saved(mut self, saved: Option<&DestinationConfig>) -> Self {
        if let (Self::S3 { access_key_id, secret_access_key, .. }, Some(Self::S3 { access_key_id: old_id, secret_access_key: old_secret, .. })) =
            (&mut self, saved)
        {
            if access_key_id.trim().is_empty() {
                *access_key_id = old_id.clone();
            }
            if secret_access_key.is_empty() {
                *secret_access_key = old_secret.clone();
            }
        }
        self
    }

    /// Refuse what cannot work, in words a person can act on.
    pub fn check(&self) -> Result<(), String> {
        match self {
            Self::Folder { path } => {
                let p = Path::new(path.trim());
                if path.trim().is_empty() {
                    return Err("Choose a folder.".into());
                }
                if !p.is_absolute() {
                    return Err("Give the folder's full path (for example D:\\Backups\\relay or /mnt/backup/relay).".into());
                }
                Ok(())
            }
            Self::S3 { endpoint, bucket, access_key_id, secret_access_key, region, prefix, .. } => {
                let u = url::Url::parse(endpoint.trim()).map_err(|_| "The endpoint is not an address (it starts with https://).".to_string())?;
                if u.scheme() != "https" && u.scheme() != "http" {
                    return Err("The endpoint starts with https:// (or http:// for a server on your own network).".into());
                }
                if u.host_str().is_none() {
                    return Err("The endpoint has no host name.".into());
                }
                if !u.username().is_empty() || u.password().is_some() {
                    return Err("Put the access key in its own fields, not in the address.".into());
                }
                if bucket.trim().is_empty() || !bucket.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_') {
                    return Err("The bucket name uses letters, digits, dots and dashes.".into());
                }
                if region.trim().is_empty() {
                    return Err("Give the region (us-east-1 when the provider does not use regions).".into());
                }
                if prefix.split('/').any(|c| c == ".." || c == ".") {
                    return Err("The folder inside the bucket cannot contain . or .. parts.".into());
                }
                if access_key_id.trim().is_empty() || secret_access_key.is_empty() {
                    return Err("Give the access key ID and the secret access key.".into());
                }
                Ok(())
            }
            Self::Server { url } => {
                let u = url::Url::parse(url.trim()).map_err(|_| "The server address starts with https://.".to_string())?;
                if u.scheme() != "https" && u.scheme() != "http" {
                    return Err("The server address starts with https://.".into());
                }
                if u.host_str().is_none() || !u.username().is_empty() || u.password().is_some() {
                    return Err("The server address is just https:// and its name.".into());
                }
                Ok(())
            }
        }
    }
}

/// `AKIA…WXYZ`: enough to recognise, not enough to use.
fn mask(s: &str) -> String {
    let s = s.trim();
    if s.len() <= 4 {
        return "set".into();
    }
    format!("…{}", &s[s.len() - 4..])
}

/// A destination ready to use.
pub enum Destination {
    Folder(FolderDest),
    S3(super::s3::S3Dest),
    Server(super::peer::ServerDest),
    /// Tests only: a folder that fails every call while `fail` is set, then recovers.
    #[cfg(test)]
    Flaky(FolderDest, std::sync::Arc<std::sync::atomic::AtomicBool>),
}

impl Destination {
    /// Build from a saved configuration. `server_seed` signs requests to a HumanityOS server.
    pub fn from_config(cfg: &DestinationConfig, server_seed: Option<&[u8; 32]>) -> Result<Self, String> {
        cfg.check()?;
        Ok(match cfg {
            DestinationConfig::Folder { path } => Self::Folder(FolderDest::new(PathBuf::from(path.trim()))),
            DestinationConfig::S3 { .. } => Self::S3(super::s3::S3Dest::new(cfg)?),
            DestinationConfig::Server { url } => {
                let seed = server_seed.ok_or("this server has no identity key yet")?;
                Self::Server(super::peer::ServerDest::new(url.trim(), seed)?)
            }
        })
    }

    pub async fn put(&self, name: &str, bytes: Vec<u8>) -> Result<(), String> {
        check_name(name)?;
        match self {
            Self::Folder(f) => f.put(name, bytes).await,
            Self::S3(s) => s.put(name, bytes).await,
            Self::Server(s) => s.put(name, bytes).await,
            #[cfg(test)]
            Self::Flaky(f, fail) => {
                if fail.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err("the test destination is down".into());
                }
                f.put(name, bytes).await
            }
        }
    }

    /// Ok(None) when there is no such object.
    pub async fn get(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        check_name(name)?;
        match self {
            Self::Folder(f) => f.get(name).await,
            Self::S3(s) => s.get(name).await,
            Self::Server(s) => s.get(name).await,
            #[cfg(test)]
            Self::Flaky(f, fail) => {
                if fail.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err("the test destination is down".into());
                }
                f.get(name).await
            }
        }
    }

    pub async fn list(&self) -> Result<Vec<Listed>, String> {
        match self {
            Self::Folder(f) => f.list().await,
            Self::S3(s) => s.list().await,
            Self::Server(s) => s.list().await,
            #[cfg(test)]
            Self::Flaky(f, fail) => {
                if fail.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err("the test destination is down".into());
                }
                f.list().await
            }
        }
    }

    /// Deleting what is not there is not an error.
    pub async fn delete(&self, name: &str) -> Result<(), String> {
        check_name(name)?;
        match self {
            Self::Folder(f) => f.delete(name).await,
            Self::S3(s) => s.delete(name).await,
            Self::Server(s) => s.delete(name).await,
            #[cfg(test)]
            Self::Flaky(f, fail) => {
                if fail.load(std::sync::atomic::Ordering::SeqCst) {
                    return Err("the test destination is down".into());
                }
                f.delete(name).await
            }
        }
    }
}

/// Object names are this feature's own, made of `[A-Za-z0-9._-]` parts joined by `/`; anything
/// else (an absolute path, `..`) is refused before it reaches a folder or a request.
pub fn check_name(name: &str) -> Result<(), String> {
    let ok = !name.is_empty()
        && name.len() <= 200
        && name.split('/').all(|c| {
            !c.is_empty() && c != "." && c != ".." && c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
        });
    if ok {
        Ok(())
    } else {
        Err("not a backup object name".into())
    }
}

/// A folder destination. Each object is written to a temporary name beside its own, flushed to
/// the disk, and only then renamed into place, so a crash or a full disk leaves either the old
/// object, the new one, or a `.part-` file that listing ignores, never a torn object.
#[derive(Clone)]
pub struct FolderDest {
    root: PathBuf,
}

const PART: &str = ".part-";

impl FolderDest {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        name.split('/').fold(self.root.clone(), |p, c| p.join(c))
    }

    pub async fn put(&self, name: &str, bytes: Vec<u8>) -> Result<(), String> {
        let path = self.path(name);
        let root = self.root.clone();
        blocking(move || write_whole(&root, &path, &bytes)).await
    }

    pub async fn get(&self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let path = self.path(name);
        blocking(move || match std::fs::read(&path) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("reading the backup folder: {e}")),
        })
        .await
    }

    pub async fn list(&self) -> Result<Vec<Listed>, String> {
        let root = self.root.clone();
        blocking(move || {
            let mut out = Vec::new();
            if !root.exists() {
                return Ok(out);
            }
            walk(&root, "", &mut out).map_err(|e| format!("listing the backup folder: {e}"))?;
            Ok(out)
        })
        .await
    }

    pub async fn delete(&self, name: &str) -> Result<(), String> {
        let path = self.path(name);
        let root = self.root.clone();
        blocking(move || {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("deleting from the backup folder: {e}")),
            }
            // Remove the folders it leaves empty (a snapshot's, a generation's), never the root.
            let mut dir = path.parent().map(Path::to_path_buf);
            while let Some(d) = dir {
                if d == root || !d.starts_with(&root) || std::fs::remove_dir(&d).is_err() {
                    break;
                }
                dir = d.parent().map(Path::to_path_buf);
            }
            Ok(())
        })
        .await
    }
}

fn write_whole(root: &Path, path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let parent = path.parent().ok_or("no folder")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("making the backup folder {}: {e}", parent.display()))?;
    let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let tmp = parent.join(format!("{file}{PART}{:08x}", rand::random::<u32>()));
    let res = (|| -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)?;
        sync_dir(parent);
        Ok(())
    })();
    if let Err(e) = res {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("writing to the backup folder {}: {e}", root.display()));
    }
    Ok(())
}

/// Flush a folder's entry list to the disk after a rename (unix; Windows has no such call).
pub fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(d) = std::fs::File::open(dir) {
        let _ = d.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

fn walk(dir: &Path, prefix: &str, out: &mut Vec<Listed>) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        let name = e.file_name().to_string_lossy().to_string();
        let full = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
        let ft = e.file_type()?;
        if ft.is_dir() {
            walk(&e.path(), &full, out)?;
        } else if ft.is_file() && !name.contains(PART) {
            out.push(Listed { name: full, size: e.metadata()?.len() });
        }
    }
    Ok(())
}

/// Run file work off the async threads.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(f).await.map_err(|e| format!("file task failed: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder destination never shows a half-written object: one cut off mid-write (left as
    /// a `.part-` file, as a crash would leave it) is not listed and not read, the whole object
    /// replaces the old one atomically, and names that could leave the folder are refused.
    ///
    /// Seen red 2026-10-22 with `.part-` files listed: "assertion `left == right` failed: only
    /// whole objects are listed / left: [.., Listed { name: \"g/seg-b.part-1234\", size: 14 }]".
    #[tokio::test]
    async fn a_folder_destination_never_shows_a_torn_object() {
        let dir = crate::test_temp::dir("bs_folder");
        let d = Destination::Folder(FolderDest::new(dir.to_path_buf()));
        d.put("g/seg-a", b"first".to_vec()).await.unwrap();
        d.put("g/seg-a", b"second".to_vec()).await.unwrap();
        std::fs::write(dir.join("g").join(format!("seg-b{PART}1234")), b"half of an obj").unwrap();
        let list = d.list().await.unwrap();
        assert_eq!(list, vec![Listed { name: "g/seg-a".into(), size: 6 }], "only whole objects are listed");
        assert_eq!(d.get("g/seg-a").await.unwrap().unwrap(), b"second");
        assert_eq!(d.get("g/seg-b").await.unwrap(), None);
        for bad in ["../x", "/etc/passwd", "a//b", "a/./b", "a\\b", "C:x"] {
            assert!(d.put(bad, vec![1]).await.is_err(), "{bad} was accepted");
        }
        d.delete("g/seg-a").await.unwrap();
        d.delete("g/seg-a").await.unwrap();
        assert!(!dir.join("g").join("seg-a").exists());
    }

    /// Seen red 2026-10-22 with the secret put into `view`: the shown text carried
    /// `"secret":"SUPER-SECRET-VALUE"`.
    #[test]
    fn the_secret_never_leaves_the_server() {
        let cfg = DestinationConfig::S3 {
            endpoint: "https://s3.example.com".into(),
            region: "us-east-1".into(),
            bucket: "b".into(),
            prefix: "relay".into(),
            access_key_id: "AKIDEXAMPLE1234".into(),
            secret_access_key: "SUPER-SECRET-VALUE".into(),
            path_style: true,
        };
        let shown = format!("{} {:?} {}", cfg.view(), cfg, cfg.summary());
        assert!(!shown.contains("SUPER-SECRET-VALUE"), "{shown}");
        assert!(!shown.contains("AKIDEXAMPLE1234"), "{shown}");
        assert!(cfg.view()["secret_set"].as_bool().unwrap());
        // An edit that leaves both blank keeps both.
        let edit = DestinationConfig::S3 {
            endpoint: "https://s3.example.com".into(),
            region: "us-east-1".into(),
            bucket: "b2".into(),
            prefix: "".into(),
            access_key_id: "".into(),
            secret_access_key: "".into(),
            path_style: true,
        };
        match edit.merged_with_saved(Some(&cfg)) {
            DestinationConfig::S3 { access_key_id, secret_access_key, bucket, .. } => {
                assert_eq!((access_key_id.as_str(), secret_access_key.as_str(), bucket.as_str()), ("AKIDEXAMPLE1234", "SUPER-SECRET-VALUE", "b2"));
            }
            _ => unreachable!(),
        }
        let in_url = DestinationConfig::S3 {
            endpoint: "https://AKID:secret@s3.example.com".into(),
            region: "r".into(),
            bucket: "b".into(),
            prefix: "".into(),
            access_key_id: "a".into(),
            secret_access_key: "s".into(),
            path_style: true,
        };
        assert!(in_url.check().is_err(), "a key in the address is refused");
    }
}
