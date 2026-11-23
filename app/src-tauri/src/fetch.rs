//! The translation packs from GitHub, so that a player has nothing to download or unpack.
//!
//! The packs are kept in one repository on GitHub, `bquenin/ventris-packs`. When the app starts it
//! gets the whole repository as one zip archive, the one GitHub offers under **Code**, **Download
//! ZIP**, and keeps its packs unpacked on this computer. This is the only place the app uses the
//! network, and this address is the only one it asks. The engine and the command-line program never
//! use the network.
//!
//! The copy is kept in the folder the system gives the app for data it can get again, under `packs/`:
//!
//! - `copy-…/` a copy of the packs, laid out as in the repository (the archive's one top folder is
//!   left out). Each refresh that downloads unpacks into a folder of its own, with a name no other
//!   refresh uses, and never writes into a folder that is already there.
//! - `record.json` which of those folders is the current copy, where it came from, when, and the tag
//!   GitHub gave that version (its ETag). It is always written whole, to a file beside it that then
//!   takes its place, so it names either the old copy or the new one, and both are complete.
//! - `lock` a file that one refresh at a time holds, across every copy of the app that is running.
//!
//! Putting a new copy in place is writing the record that names it: no folder is moved or renamed,
//! so a file of the old copy that is open, which on Windows stops a folder from being renamed, does
//! not get in the way, and an app stopped at any moment leaves the record naming a whole copy. The
//! copy before the current one is kept until the next refresh, for a copy of the app that is still
//! reading it; older ones, and what a stopped refresh left, are then removed, or left for a later
//! refresh while a file in them is open.
//!
//! The tag is sent back on the next start. When the repository has not changed since, GitHub answers
//! "not modified" and nothing is downloaded. A failed or interrupted refresh leaves the current copy
//! as it was.
//!
//! An archive comes from the network, so it is unpacked with care. Only the folders that hold a
//! `pack.toml`, and what is in them, are unpacked; the rest of the repository is left out. A file
//! whose path would land outside the copy's folder is refused, and so is a name that some system
//! cannot hold, two names that only differ in capital letters, and an archive larger, or with more
//! files, than any packs repository needs. A refusal is the same on every system.
//!
//! What a failed refresh says is shown on the page: it is a plain sentence, and never names a file on
//! this computer. What the system said goes to the standard error, for whoever runs the app from a
//! terminal.

use std::collections::BTreeMap;
use std::fs::{self, File, TryLockError};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use ureq::tls::{RootCerts, TlsConfig};
use ventris::archive;
use ventris::manifest::MANIFEST_FILE;

/// The whole packs repository, as one zip archive. GitHub sends it on to `codeload.github.com`.
pub const PACKS_URL: &str = "https://github.com/bquenin/ventris-packs/archive/refs/heads/main.zip";

/// An environment variable that gives another address in place of `PACKS_URL`, to try the app
/// against a copy of the packs served from this computer. It takes an `https://` address, or an
/// `http://` one on this computer itself (`LOOPBACK`).
pub const URL_VARIABLE: &str = "VENTRIS_PACKS_URL";

/// The hosts an `http://` address in `URL_VARIABLE` may name: this computer, and no other.
const LOOPBACK: [&str; 3] = ["127.0.0.1", "localhost", "[::1]"];

const MIB: u64 = 1024 * 1024;
/// The largest archive that is downloaded. The repository is about 2 MB today.
pub const MAX_DOWNLOAD: u64 = 100 * MIB;
/// The most its files may take once unpacked, as the archive states their sizes; about 3.5 MB today.
/// Text packs well, so this is more than the download's limit, but it stops a small archive that
/// unpacks to fill the disk.
pub const MAX_UNPACKED: u64 = 512 * MIB;
/// The most files an archive may hold. The repository has about 1,800 today.
pub const MAX_ENTRIES: usize = 20_000;

/// How long to wait for the connection to be made, and for the whole answer to arrive.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const WHOLE_TIMEOUT: Duration = Duration::from_secs(180);

/// What the page is told when a copy cannot be written, whatever the system said.
const NOT_WRITTEN: &str = "the copy of the packs could not be written on this computer";
/// When the copy written does not hold every file the archive gave for it.
const NOT_WHOLE: &str = "the copy of the packs could not be written whole on this computer";
/// When the archive, once downloaded, cannot be read back from the disk.
const NOT_READ: &str = "the archive of the packs could not be read on this computer";
/// When the archive is one, but cannot be unpacked: a file in it does not match its checksum, or
/// its list of files is cut short.
const DAMAGED: &str = "the archive of the packs that was received is damaged, and was not used";

/// The beginning of the name of a folder that holds a copy, and of a download under way.
const COPY_PREFIX: &str = "copy-";
const DOWNLOAD_PREFIX: &str = "download-";
const RECORD_FILE: &str = "record.json";
/// What the record is written to before it takes its place: `record.json.<unique>.part`.
const RECORD_PART_PREFIX: &str = "record.json.";
const LOCK_FILE: &str = "lock";
/// What the app kept before copies had folders of their own: one folder, `current/`, that a new
/// copy was renamed into, by way of `incoming/` and `previous/`. Removed at the next refresh.
const OLD_LAYOUT: [&str; 4] = ["current", "incoming", "previous", "download.zip.part"];

/// Held while a copy is fetched and put in place, so that two refreshes of this app cannot cross.
/// The lock file does the same between this app and another copy of it.
static FETCHING: Mutex<()> = Mutex::new(());

/// Where the packs are fetched from.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub url: String,
    /// Whether every request, redirects included, must be made over HTTPS. Only an address on this
    /// computer, given in `URL_VARIABLE`, may be fetched without it.
    pub https_only: bool,
}

/// Where the packs are fetched from: `PACKS_URL`, unless `URL_VARIABLE` names another. An address
/// there that is neither HTTPS nor on this computer is refused, in words.
pub fn packs_source() -> Result<Source, String> {
    source_from(std::env::var(URL_VARIABLE).ok())
}

fn source_from(variable: Option<String>) -> Result<Source, String> {
    let Some(url) = variable.map(|url| url.trim().to_string()).filter(|url| !url.is_empty()) else {
        return Ok(Source { url: PACKS_URL.to_string(), https_only: true });
    };
    let (scheme, rest) = url.split_once("://").unwrap_or(("", ""));
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if scheme.eq_ignore_ascii_case("https") && !authority.is_empty() {
        return Ok(Source { url, https_only: true });
    }
    if scheme.eq_ignore_ascii_case("http") && is_loopback(authority) {
        return Ok(Source { url, https_only: false });
    }
    // The address is not repeated: what the page shows is a sentence of the app's own.
    Err(format!(
        "the address in {URL_VARIABLE} was not used: it must start with https://, or with http:// for this computer \
         alone (127.0.0.1, localhost or [::1])"
    ))
}

/// Whether the host and port of an address are this computer's: `127.0.0.1:8765`, `[::1]`. One with
/// a user's name before an `@` is not, since what follows the `@` is the host.
fn is_loopback(authority: &str) -> bool {
    if authority.contains('@') {
        return false;
    }
    let host = if authority.starts_with('[') {
        authority.split_inclusive(']').next().unwrap_or("")
    } else {
        authority.split(':').next().unwrap_or("")
    };
    LOOPBACK.iter().any(|known| host.eq_ignore_ascii_case(known))
}

/// What is remembered about the current copy.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct Record {
    /// The address it was fetched from. A tag is only sent back to the address that gave it.
    url: String,
    /// GitHub's tag for that version of the archive, when it gave one.
    etag: Option<String>,
    /// When it was downloaded, and when it was last found to be up to date, in seconds since 1970.
    downloaded: u64,
    checked: u64,
    /// The folder beside the record that holds the copy: `copy-…`. A record written before copies
    /// had folders of their own has none, and is taken as no copy at all.
    #[serde(default)]
    folder: Option<String>,
}

/// The current copy of the packs, if there is one.
#[derive(Debug, PartialEq)]
pub struct Kept {
    pub folder: PathBuf,
    /// When it was last found to be up to date, in seconds since 1970; 0 when that is not known.
    pub checked: u64,
}

/// What a refresh did.
#[derive(Debug, PartialEq)]
pub enum Refreshed {
    /// The current copy is the latest; nothing was downloaded.
    Unchanged,
    /// A new copy was downloaded and put in place.
    Replaced,
    /// Another copy of the app was refreshing the packs. This one waited for it to be done, and
    /// fetched nothing: the copy is the one the other put in place.
    ByAnother,
}

/// The place the copies are kept, inside the app's folder for cached data.
pub struct Store {
    root: PathBuf,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

/// A name no other refresh uses, in this app or in another copy of it: the process, the time, and
/// a count.
fn unique() -> String {
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_nanos());
    format!("{}-{nanos}-{}", std::process::id(), COUNT.fetch_add(1, Ordering::Relaxed))
}

/// Say what the system said on the standard error, and give the page's sentence in its place.
fn told(sentence: &str, detail: impl std::fmt::Display) -> String {
    eprintln!("ventris: {sentence}: {detail}");
    sentence.to_string()
}

/// Whether a name in the store's folder is one a copy's folder may have. A record could be edited
/// by hand: the folder it names is only taken from beside it.
fn is_copy_name(name: &str) -> bool {
    name.len() > COPY_PREFIX.len() && name.starts_with(COPY_PREFIX) && !name.contains(['/', '\\', ':'])
}

impl Store {
    pub fn new(cache: &Path) -> Self {
        Store { root: cache.join("packs") }
    }

    fn record_file(&self) -> PathBuf {
        self.root.join(RECORD_FILE)
    }

    fn record(&self) -> Option<Record> {
        fs::read_to_string(self.record_file()).ok().and_then(|content| serde_json::from_str(&content).ok())
    }

    /// Write the record whole to a file beside it, then put it in place, so it is never half written.
    fn save_record(&self, record: &Record) -> Result<(), String> {
        let file = self.record_file();
        let written = self.root.join(format!("{RECORD_PART_PREFIX}{}.part", unique()));
        let content = serde_json::to_string_pretty(record).map_err(|error| told(NOT_WRITTEN, error))?;
        fs::write(&written, content).map_err(|error| told(NOT_WRITTEN, format!("{}: {error}", written.display())))?;
        fs::rename(&written, &file).map_err(|error| {
            let _ = fs::remove_file(&written);
            told(NOT_WRITTEN, format!("{}: {error}", file.display()))
        })
    }

    /// The record, and the folder of the copy it names, when that folder is there.
    fn current(&self) -> Option<(Record, PathBuf)> {
        let record = self.record()?;
        let folder = self.root.join(record.folder.as_deref().filter(|name| is_copy_name(name))?);
        folder.is_dir().then_some((record, folder))
    }

    /// The current copy, if there is one.
    pub fn kept(&self) -> Option<Kept> {
        self.current().map(|(record, folder)| Kept { folder, checked: record.checked })
    }

    /// Fetch the packs from `source` and keep them, unless the current copy is already the latest.
    /// On failure the current copy is left as it was, and the error says why in words.
    ///
    /// When another copy of the app is refreshing them, this one waits until it is done and fetches
    /// nothing: the copy is then the one the other put in place.
    pub fn refresh(&self, source: &Source) -> Result<Refreshed, String> {
        let _held = FETCHING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        fs::create_dir_all(&self.root).map_err(|error| told(NOT_WRITTEN, error))?;
        let lock = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.root.join(LOCK_FILE))
            .map_err(|error| told(NOT_WRITTEN, error))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                // Released when the other is done, or when it stops, whatever the way.
                lock.lock().map_err(|error| told(NOT_WRITTEN, error))?;
                return Ok(Refreshed::ByAnother);
            }
            // Some file systems cannot lock a file. The refresh is then held to this app alone.
            Err(TryLockError::Error(error)) => eprintln!("ventris: the packs' lock file could not be locked: {error}"),
        }
        let before = self.current().map(|(record, _)| record);
        let refreshed = self.fetch(source, before.clone());
        let after = self.current().map(|(record, _)| record);
        let keep: Vec<String> = [before, after].into_iter().flatten().filter_map(|record| record.folder).collect();
        self.clear_away(&keep);
        refreshed
        // The lock is let go when its file is closed, here.
    }

    /// The download, the unpacking, and the record that puts the new copy in place. `before` is the
    /// record of the current copy.
    fn fetch(&self, source: &Source, before: Option<Record>) -> Result<Refreshed, String> {
        let before = before.filter(|record| record.url == source.url);
        let etag = before.as_ref().and_then(|record| record.etag.clone());
        let unique = unique();
        let part = self.root.join(format!("{DOWNLOAD_PREFIX}{unique}.zip.part"));
        let fetched = download(source, etag.as_deref(), &part);
        let new_etag = match (fetched, before) {
            (Ok(Downloaded::NotModified), Some(record)) => {
                self.save_record(&Record { checked: now(), ..record })?;
                return Ok(Refreshed::Unchanged);
            }
            (Ok(Downloaded::Archive { etag }), _) => etag,
            // "Not modified" is only answered to a tag, and a tag is only sent for a copy there is.
            (Ok(Downloaded::NotModified), None) => {
                let _ = fs::remove_file(&part);
                return Err(format!(
                    "{} answered 304 Not Modified to a request that did not ask it",
                    host(&source.url)
                ));
            }
            (Err(error), _) => {
                let _ = fs::remove_file(&part);
                return Err(error);
            }
        };

        let name = format!("{COPY_PREFIX}{unique}");
        let folder = self.root.join(&name);
        let unpacked = unpack(&part, &folder);
        let _ = fs::remove_file(&part);
        let time = now();
        let record =
            Record { url: source.url.clone(), etag: new_etag, downloaded: time, checked: time, folder: Some(name) };
        if let Err(error) = unpacked.and_then(|_| self.save_record(&record)) {
            let _ = fs::remove_dir_all(&folder);
            return Err(error);
        }
        Ok(Refreshed::Replaced)
    }

    /// Remove what is beside the copies in `keep`: older copies, what a refresh that stopped left,
    /// and the folders of the old layout. Only called while the lock is held, so that no refresh is
    /// writing any of it. What cannot be removed now, such as a folder with a file open in it, is
    /// left for a later refresh.
    fn clear_away(&self, keep: &[String]) {
        let Ok(entries) = fs::read_dir(&self.root) else { return };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let ours = (name.starts_with(COPY_PREFIX) && !keep.contains(&name))
                || name.starts_with(DOWNLOAD_PREFIX)
                || (name.starts_with(RECORD_PART_PREFIX) && name.ends_with(".part"))
                || OLD_LAYOUT.contains(&name.as_str());
            if !ours {
                continue;
            }
            let path = entry.path();
            let removed = if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                fs::remove_dir_all(&path)
            } else {
                fs::remove_file(&path)
            };
            if let Err(error) = removed {
                eprintln!("ventris: {} was left for a later refresh: {error}", path.display());
            }
        }
    }

    /// Name a folder beside the record as the current copy, as a refresh does: for tests elsewhere
    /// in the app that need a copy without a server.
    #[cfg(test)]
    pub(crate) fn adopt(&self, folder: &str, checked: u64) {
        let record = Record { folder: Some(folder.to_string()), checked, ..Record::default() };
        self.save_record(&record).unwrap();
    }
}

/// What a download brought.
enum Downloaded {
    /// The archive the server holds is the one with the tag that was sent.
    NotModified,
    /// The archive, now in the file given, with the server's tag for it.
    Archive { etag: Option<String> },
}

/// The host of an address, to name it in a message: `github.com`.
fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

fn agent(https_only: bool) -> ureq::Agent {
    // The system's own trusted roots, not a list compiled into the app: the system keeps them up
    // to date, and a root an employer or a school added to it is trusted as the user's browser does.
    let tls = TlsConfig::builder().root_certs(RootCerts::PlatformVerifier).build();
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_global(Some(WHOLE_TIMEOUT))
        // Nothing about the user or the computer: only what the program is.
        .user_agent(concat!("ventris-app/", env!("CARGO_PKG_VERSION")))
        .http_status_as_error(false)
        .max_redirects(5)
        // Checked before each request, so a redirect to an address without HTTPS is not followed.
        .https_only(https_only)
        .tls_config(tls)
        .build();
    ureq::Agent::new_with_config(config)
}

/// Ask the source for the archive, sending the tag of the current copy when there is one, and write
/// the archive to `to`.
fn download(source: &Source, etag: Option<&str>, to: &Path) -> Result<Downloaded, String> {
    let url = source.url.as_str();
    let mut request = agent(source.https_only).get(url);
    if let Some(etag) = etag {
        request = request.header("If-None-Match", etag);
    }
    // In words for the page; what the library said goes to the standard error.
    let response = request.call().map_err(|error| {
        let sentence = match &error {
            ureq::Error::Timeout(_) => format!("{} took too long to answer", host(url)),
            ureq::Error::HostNotFound => format!("{} could not be found", host(url)),
            ureq::Error::Tls(_) | ureq::Error::Rustls(_) => {
                format!("the connection to {} could not be made secure", host(url))
            }
            ureq::Error::RequireHttpsOnly(_) => {
                format!(
                    "{} sent the download on to an address without a secure connection, which was not followed",
                    host(url)
                )
            }
            _ => format!("{} could not be reached", host(url)),
        };
        told(&sentence, error)
    })?;
    let status = response.status();
    if status.as_u16() == 304 && etag.is_some() {
        return Ok(Downloaded::NotModified);
    }
    if status.as_u16() != 200 {
        let reason = status.canonical_reason().map(|reason| format!(" {reason}")).unwrap_or_default();
        return Err(format!("{} answered {}{reason}", host(url), status.as_u16()));
    }
    let etag = response.headers().get("etag").and_then(|value| value.to_str().ok()).map(str::to_string);

    let mut file = File::create(to).map_err(|error| told(NOT_WRITTEN, format!("{}: {error}", to.display())))?;
    let mut body = response.into_body().into_reader();
    let (mut size, mut start) = (0u64, Vec::<u8>::new());
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let read = match body.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(told("the download was cut short", error)),
        };
        size += read as u64;
        if size > MAX_DOWNLOAD {
            return Err(format!(
                "the archive is larger than {} MB, more than the packs can be, and was not used",
                MAX_DOWNLOAD / MIB
            ));
        }
        if start.len() < 2 {
            start.extend(buffer[..read].iter().take(2 - start.len()));
        }
        file.write_all(&buffer[..read]).map_err(|error| told(NOT_WRITTEN, error))?;
    }
    // Every zip archive starts with "PK". A network that wants the user to sign in first answers
    // every address with its sign-in page, which is what comes in place of the archive most often.
    if start != b"PK" {
        return Err(format!(
            "what {} sent is not an archive of the packs: it may be the sign-in page of a network that asks you to \
             sign in first, such as a hotel's Wi-Fi",
            host(url)
        ));
    }
    file.sync_all().map_err(|error| told(NOT_WRITTEN, error))?;
    Ok(Downloaded::Archive { etag })
}

/// The names Windows keeps for devices, which no file or folder can have there, with or without an
/// ending: `NUL`, `nul.txt`, `COM1.md`.
fn is_device_name(part: &str) -> bool {
    let base = part.split('.').next().unwrap_or(part).trim_end_matches(' ').to_ascii_uppercase();
    let numbered = |prefix: &str| {
        base.strip_prefix(prefix).is_some_and(|rest| {
            matches!(rest, "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")
        })
    };
    matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL") || numbered("COM") || numbered("LPT")
}

/// A name with its control characters written out, so that a message can show it.
fn shown(name: &str) -> String {
    name.chars().map(|c| if c.is_control() { format!("\\u{{{:04x}}}", c as u32) } else { c.to_string() }).collect()
}

/// Why one part of a path, a file's or a folder's name, cannot be held by every system, if it
/// cannot. The rules are Windows', which are the narrowest, and are the same on every system, so
/// that a mistake in the packs repository is refused the same way everywhere.
fn not_portable(part: &str) -> Option<String> {
    if let Some(c) = part.chars().find(|c| c.is_control()) {
        return Some(format!("its name holds a control character ({})", shown(&c.to_string())));
    }
    if let Some(c) = part.chars().find(|c| r#"\?*<>|":"#.contains(*c)) {
        return Some(format!("its name holds {c}, which Windows does not allow in a name"));
    }
    if part.ends_with('.') || part.ends_with(' ') {
        return Some(format!("“{part}” ends in a dot or a space, which Windows does not keep in a name"));
    }
    if is_device_name(part) {
        return Some(format!("“{part}” is a name Windows keeps for a device"));
    }
    None
}

/// Unpack the archive at `archive` into `into`, which must not exist yet, leaving out the one
/// folder at the top that every file is in (`ventris-packs-main/`). Gives the number of files.
///
/// Only the folders that hold a `pack.toml`, and what is in them, are unpacked. The whole archive is
/// refused when one of its files would land outside `into`, when a file to unpack has a name some
/// system cannot hold or one that differs from another's only in capital letters, or when it holds
/// more files, or more bytes, than the limits above. A link in the archive is written as the small
/// file the archive holds for it, never as a link. Once written, the files in `into` are counted:
/// a copy that is not whole is refused.
pub fn unpack(archive: &Path, into: &Path) -> Result<usize, String> {
    let refused = |why: String| Err(format!("the archive of the packs was not used: {why}"));
    let entries = archive::entries(archive).map_err(|error| match error {
        ventris::Error::Io(error) => told(NOT_READ, error),
        other => told(DAMAGED, other),
    })?;
    if entries.len() > MAX_ENTRIES {
        return refused(format!("it holds {} files, more than the {MAX_ENTRIES} the packs can have", entries.len()));
    }
    let total: u64 = entries.iter().map(|entry| entry.size).sum();
    if total > MAX_UNPACKED {
        return refused(format!("it would take {} MB unpacked, more than the packs can", total / MIB));
    }
    let top = entries.first().and_then(|entry| entry.name.split_once('/')).map(|(top, _)| top.to_string());
    let Some(top) = top else { return refused("it does not hold one folder with the packs in it".to_string()) };

    // Every file's path inside the top folder, as its parts.
    let mut paths: Vec<(&archive::Entry, Vec<&str>)> = Vec::with_capacity(entries.len());
    for entry in &entries {
        let Some(inside) = entry.name.strip_prefix(&top).and_then(|rest| rest.strip_prefix('/')) else {
            return refused(format!("{} is not in the folder {top}", shown(&entry.name)));
        };
        // Parts are split at `/`, and the archive reader reads a backslash as one too; what is left
        // to lead out is an empty part (a root, or `//`), `.` and `..`.
        let parts: Vec<&str> = inside.split('/').collect();
        if parts.iter().any(|part| part.is_empty() || *part == "." || *part == "..") {
            return refused(format!("{} would be written outside the folder of the packs", shown(&entry.name)));
        }
        paths.push((entry, parts));
    }

    // The packs' folders: those with a pack.toml, but hidden ones, which the app never reads.
    let packs: Vec<&[&str]> = paths
        .iter()
        .filter(|(_, parts)| parts.last() == Some(&MANIFEST_FILE))
        .map(|(_, parts)| &parts[..parts.len() - 1])
        .filter(|folder| !folder.iter().any(|part| part.starts_with('.')))
        .collect();
    if packs.is_empty() {
        return refused(format!("no folder in it has a {MANIFEST_FILE}, the file that makes a folder a pack"));
    }
    let chosen: Vec<&(&archive::Entry, Vec<&str>)> =
        paths.iter().filter(|(_, parts)| packs.iter().any(|pack| parts[..parts.len() - 1].starts_with(pack))).collect();

    // Each name a file or a folder to unpack has, by its lowercase form: two that are the same once
    // lowercased would be one on Windows and macOS, and two elsewhere.
    let mut names: BTreeMap<String, (String, bool)> = BTreeMap::new();
    for (_, parts) in &chosen {
        let path = parts.join("/");
        if let Some(why) = parts.iter().find_map(|part| not_portable(part)) {
            return refused(format!("{} cannot be unpacked on every system: {why}", shown(&path)));
        }
        for end in 1..=parts.len() {
            let (name, file) = (parts[..end].join("/"), end == parts.len());
            match names.get(&name.to_lowercase()) {
                None => {
                    names.insert(name.to_lowercase(), (name, file));
                }
                Some((other, _)) if *other != name => {
                    return refused(format!("{} and {} differ only in capital letters", shown(other), shown(&name)));
                }
                Some((_, other_file)) if file && *other_file => {
                    return refused(format!("{} is in it twice", shown(&name)));
                }
                Some((_, other_file)) if file != *other_file => {
                    return refused(format!("{} is both a file and a folder", shown(&name)));
                }
                Some(_) => {}
            }
        }
    }

    fs::create_dir_all(into).map_err(|error| told(NOT_WRITTEN, format!("{}: {error}", into.display())))?;
    for (entry, parts) in &chosen {
        let path = parts.iter().fold(into.to_path_buf(), |path, part| path.join(part));
        let data = archive::read(archive, entry).map_err(|error| match error {
            ventris::Error::Io(error) => told(NOT_READ, error),
            other => told(DAMAGED, other),
        })?;
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder).map_err(|error| told(NOT_WRITTEN, format!("{}: {error}", folder.display())))?;
        }
        fs::write(&path, data).map_err(|error| told(NOT_WRITTEN, format!("{}: {error}", path.display())))?;
    }
    let written = count_files(into).map_err(|error| told(NOT_WRITTEN, error))?;
    if written != chosen.len() {
        return Err(told(NOT_WHOLE, format!("{written} files were written of {}", chosen.len())));
    }
    Ok(written)
}

/// How many files there are in a folder and the folders in it.
fn count_files(folder: &Path) -> io::Result<usize> {
    let mut count = 0;
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        count += if entry.file_type()?.is_dir() { count_files(&entry.path())? } else { 1 };
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};
    use std::thread;

    use super::*;

    fn manifest(game: &str) -> String {
        format!(
            "format = 1\ngame = \"{game}\"\nlanguage = \"en\"\nname = \"Trial\"\nversion = \"1\"\nauthors = [\"Ana\"]\n"
        )
    }

    /// An archive laid out as GitHub's: everything in one top folder, with folders listed too, and
    /// files beside the packs that are not unpacked.
    fn packs_archive(version: &str) -> Vec<u8> {
        let manifest = manifest("top-sfc");
        let strings = format!("[01.000]\nHello, version {version}.\n\n");
        archive::make(&[
            ("ventris-packs-main/", b"", false),
            ("ventris-packs-main/README.md", b"The packs.", false),
            ("ventris-packs-main/.github/workflows/check.yml", b"on: push", false),
            ("ventris-packs-main/top-sfc/en/", b"", false),
            ("ventris-packs-main/top-sfc/en/pack.toml", manifest.as_bytes(), true),
            ("ventris-packs-main/top-sfc/en/01.txt", strings.as_bytes(), true),
            ("ventris-packs-main/top-sfc/en/NOTES.md", b"Notes of the pack.", false),
        ])
    }

    /// The same, with `files` added to the pack's folder `top-sfc/en/`.
    fn pack_with(files: &[(&str, &[u8])]) -> Vec<u8> {
        let manifest = manifest("top-sfc");
        let mut all: Vec<(String, Vec<u8>)> = vec![
            ("ventris-packs-main/README.md".to_string(), b"The packs.".to_vec()),
            ("ventris-packs-main/top-sfc/en/pack.toml".to_string(), manifest.into_bytes()),
        ];
        all.extend(files.iter().map(|(name, data)| (format!("ventris-packs-main/top-sfc/en/{name}"), data.to_vec())));
        let listed: Vec<(&str, &[u8], bool)> =
            all.iter().map(|(name, data)| (name.as_str(), data.as_slice(), false)).collect();
        archive::make(&listed)
    }

    fn unpacked(archive: &[u8]) -> (tempfile::TempDir, Result<usize, String>) {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("packs.zip");
        fs::write(&path, archive).unwrap();
        let result = unpack(&path, &folder.path().join("out"));
        (folder, result)
    }

    fn refusal(why: &str) -> String {
        format!("the archive of the packs was not used: {why}")
    }

    #[test]
    fn the_address_can_be_given_by_the_environment() {
        let built_in = Source { url: PACKS_URL.to_string(), https_only: true };
        assert_eq!(source_from(None), Ok(built_in.clone()));
        assert_eq!(source_from(Some("  ".to_string())), Ok(built_in));
        let local = |url: &str| Ok(Source { url: url.to_string(), https_only: false });
        // The owner's trial script serves the packs on this computer, without HTTPS.
        assert_eq!(
            source_from(Some(" http://127.0.0.1:8765/p.zip ".to_string())),
            local("http://127.0.0.1:8765/p.zip")
        );
        assert_eq!(source_from(Some("http://localhost/p.zip".to_string())), local("http://localhost/p.zip"));
        assert_eq!(source_from(Some("HTTP://[::1]:80/p.zip".to_string())), local("HTTP://[::1]:80/p.zip"));
        let secure = "https://example.org/packs.zip";
        assert_eq!(source_from(Some(secure.to_string())), Ok(Source { url: secure.to_string(), https_only: true }));
        assert_eq!(host(PACKS_URL), "github.com");
        assert_eq!(host("http://127.0.0.1:8000/p.zip"), "127.0.0.1:8000");
    }

    /// An `http://` address elsewhere than this computer is refused, whatever it is dressed as, and
    /// the refusal does not repeat it.
    #[test]
    fn an_address_without_https_is_refused_unless_it_is_this_computer() {
        let said = "the address in VENTRIS_PACKS_URL was not used: it must start with https://, or with http:// for \
                    this computer alone (127.0.0.1, localhost or [::1])";
        for url in [
            "http://example.org/p.zip",
            "http://127.0.0.1.example.org/p.zip",
            "http://localhost.example.org/p.zip",
            "http://127.0.0.1@example.org/p.zip",
            "http://example.org/127.0.0.1/p.zip",
            "http://[::2]/p.zip",
            "ftp://127.0.0.1/p.zip",
            "file:///C:/packs.zip",
            "https:///p.zip",
            "127.0.0.1:8765/p.zip",
        ] {
            assert_eq!(source_from(Some(url.to_string())), Err(said.to_string()), "{url}");
        }
    }

    /// The built-in address is held to HTTPS: a request without it is not even sent, and the same
    /// check is made before each redirect is followed.
    #[test]
    fn https_only_sends_nothing_without_https() {
        let (address, asked) = serve(vec![(200, vec![], packs_archive("1"))]);
        let folder = tempfile::tempdir().unwrap();
        let to = folder.path().join("download.zip");
        let source = Source { url: format!("{address}/main.zip"), https_only: true };
        let refused = download(&source, None, &to).err().unwrap();
        assert_eq!(
            refused,
            format!(
                "{} sent the download on to an address without a secure connection, which was not followed",
                host(&source.url)
            )
        );
        assert!(asked.lock().unwrap().is_empty() && !to.exists());
        assert!(agent(true).config().https_only() && !agent(false).config().https_only());
    }

    #[test]
    fn only_the_packs_are_unpacked_without_the_top_folder() {
        let (folder, result) = unpacked(&packs_archive("1"));
        assert_eq!(result, Ok(3));
        let out = folder.path().join("out");
        assert_eq!(fs::read_to_string(out.join("top-sfc").join("en").join("pack.toml")).unwrap(), manifest("top-sfc"));
        assert_eq!(fs::read_to_string(out.join("top-sfc").join("en").join("NOTES.md")).unwrap(), "Notes of the pack.");
        // The repository's own files, beside the packs, are left out.
        assert!(!out.join("README.md").exists() && !out.join(".github").exists());
        assert!(!out.join("ventris-packs-main").exists());

        // A hidden folder with a pack.toml is not a pack the app reads; a pack.toml at the top makes
        // the whole repository one pack.
        let hidden = archive::make(&[
            ("ventris-packs-main/.trial/pack.toml", manifest("top-sfc").as_bytes(), false),
            ("ventris-packs-main/top-sfc/en/pack.toml", manifest("top-sfc").as_bytes(), false),
        ]);
        let (folder, result) = unpacked(&hidden);
        assert_eq!(result, Ok(1));
        assert!(!folder.path().join("out").join(".trial").exists());
        let whole = archive::make(&[
            ("ventris-packs-main/pack.toml", manifest("top-sfc").as_bytes(), false),
            ("ventris-packs-main/README.md", b"The pack.", false),
        ]);
        assert_eq!(unpacked(&whole).1, Ok(2));

        // An archive with no pack in it would leave nothing to list.
        let (folder, result) = unpacked(&archive::make(&[("ventris-packs-main/README.md", b"The packs.", false)]));
        assert_eq!(result, Err(refusal("no folder in it has a pack.toml, the file that makes a folder a pack")));
        assert!(!folder.path().join("out").exists());
    }

    #[test]
    fn a_path_that_leads_out_of_the_folder_is_refused() {
        for (name, why) in [
            (
                "ventris-packs-main/../outside.txt",
                "ventris-packs-main/../outside.txt would be written outside the folder of the packs",
            ),
            (
                "ventris-packs-main/top-sfc/../../outside.txt",
                "ventris-packs-main/top-sfc/../../outside.txt would be written outside the folder of the packs",
            ),
            (
                "ventris-packs-main//outside.txt",
                "ventris-packs-main//outside.txt would be written outside the folder of the packs",
            ),
            (
                "ventris-packs-main/./outside.txt",
                "ventris-packs-main/./outside.txt would be written outside the folder of the packs",
            ),
            (
                "/ventris-packs-main/outside.txt",
                "/ventris-packs-main/outside.txt is not in the folder ventris-packs-main",
            ),
            ("other/outside.txt", "other/outside.txt is not in the folder ventris-packs-main"),
            // A drive inside a pack's folder: not a name Windows can hold, so never written.
            (
                "ventris-packs-main/top-sfc/en/C:/outside.txt",
                "top-sfc/en/C:/outside.txt cannot be unpacked on every system: its name holds :, which Windows does \
                 not allow in a name",
            ),
            (
                "ventris-packs-main/top-sfc/en/C:outside.txt",
                "top-sfc/en/C:outside.txt cannot be unpacked on every system: its name holds :, which Windows does not \
                 allow in a name",
            ),
        ] {
            let archive = archive::make(&[
                ("ventris-packs-main/top-sfc/en/pack.toml", manifest("top-sfc").as_bytes(), false),
                (name, b"not here", false),
            ]);
            let (folder, result) = unpacked(&archive);
            assert_eq!(result, Err(refusal(why)), "{name}");
            // Nothing at all is written, the harmless file included.
            assert!(!folder.path().join("out").exists(), "{name}");
            assert!(!folder.path().join("outside.txt").exists(), "{name}");
        }
        // A backslash is read as a separator, as the archive reader does for every name.
        let archive = archive::make(&[("ventris-packs-main\\..\\outside.txt", b"not here", false)]);
        assert_eq!(
            unpacked(&archive).1,
            Err(refusal("ventris-packs-main/../outside.txt would be written outside the folder of the packs"))
        );
        // Files that are not in one folder.
        let (_, result) = unpacked(&archive::make(&[("README.md", b"loose", false)]));
        assert_eq!(result, Err(refusal("it does not hold one folder with the packs in it")));
    }

    /// A name in a pack that some system cannot hold is refused by name, on every system alike. The
    /// same names outside the packs' folders are not unpacked, and do not stop anything.
    #[test]
    fn a_name_some_system_cannot_hold_is_refused_inside_a_pack() {
        let character = |c: &str| format!("its name holds {c}, which Windows does not allow in a name");
        let cases: Vec<(String, String)> = vec![
            ("why?.txt".into(), character("?")),
            ("a*.txt".into(), character("*")),
            ("a<b.txt".into(), character("<")),
            ("a>b.txt".into(), character(">")),
            ("a|b.txt".into(), character("|")),
            ("say \"hi\".txt".into(), character("\"")),
            ("a:b.txt".into(), character(":")),
            ("tab\there.txt".into(), "its name holds a control character (\\u{0009})".into()),
            ("notes.".into(), "“notes.” ends in a dot or a space, which Windows does not keep in a name".into()),
            ("notes ".into(), "“notes ” ends in a dot or a space, which Windows does not keep in a name".into()),
            ("dir./01.txt".into(), "“dir.” ends in a dot or a space, which Windows does not keep in a name".into()),
            ("CON".into(), "“CON” is a name Windows keeps for a device".into()),
            ("nul.txt".into(), "“nul.txt” is a name Windows keeps for a device".into()),
            ("Com1.md".into(), "“Com1.md” is a name Windows keeps for a device".into()),
            ("LPT9".into(), "“LPT9” is a name Windows keeps for a device".into()),
            ("aux .txt".into(), "“aux .txt” is a name Windows keeps for a device".into()),
            ("COM¹.txt".into(), "“COM¹.txt” is a name Windows keeps for a device".into()),
            ("prn/01.txt".into(), "“prn” is a name Windows keeps for a device".into()),
        ];
        for (name, why) in &cases {
            let (folder, result) = unpacked(&pack_with(&[("01.txt", &b"[01.000]\nHi\n\n"[..]), (name, &b"x"[..])]));
            let shown_name = shown(&format!("top-sfc/en/{name}"));
            assert_eq!(
                result,
                Err(refusal(&format!("{shown_name} cannot be unpacked on every system: {why}"))),
                "{name}"
            );
            assert!(!folder.path().join("out").exists(), "{name}");
        }
        // Names that only look like those: kept.
        let x = &b"x"[..];
        let fine = pack_with(&[("console.txt", x), ("COM10.txt", x), ("nul-notes.md", x), ("a.b.txt", x)]);
        assert_eq!(unpacked(&fine).1, Ok(5));
        // Outside the packs, the same names are not unpacked, and do not stop anything.
        let manifest = manifest("top-sfc");
        let mut beside: Vec<(String, &[u8])> =
            cases.iter().map(|(name, _)| (format!("ventris-packs-main/docs/{name}"), x)).collect();
        beside.push(("ventris-packs-main/top-sfc/en/pack.toml".to_string(), manifest.as_bytes()));
        let listed: Vec<(&str, &[u8], bool)> =
            beside.iter().map(|(name, data)| (name.as_str(), *data, false)).collect();
        let (folder, result) = unpacked(&archive::make(&listed));
        assert_eq!(result, Ok(1));
        assert!(!folder.path().join("out").join("docs").exists());
    }

    /// Two names that are one on Windows and macOS are refused, whether files or folders, and so is
    /// a name that is both a file and a folder, or a file that is in the archive twice.
    #[test]
    fn names_that_would_merge_are_refused() {
        for (files, why) in [
            (vec!["01.txt", "01.TXT"], "top-sfc/en/01.txt and top-sfc/en/01.TXT differ only in capital letters"),
            (
                vec!["fonts/a.ttf", "Fonts/b.ttf"],
                "top-sfc/en/fonts and top-sfc/en/Fonts differ only in capital letters",
            ),
            (
                vec!["Élan.txt", "élan.txt"],
                "top-sfc/en/Élan.txt and top-sfc/en/élan.txt differ only in capital letters",
            ),
            (vec!["notes", "notes/a.md"], "top-sfc/en/notes is both a file and a folder"),
            (vec!["01.txt", "01.txt"], "top-sfc/en/01.txt is in it twice"),
        ] {
            let listed: Vec<(&str, &[u8])> = files.iter().map(|name| (*name, &b"x"[..])).collect();
            let (folder, result) = unpacked(&pack_with(&listed));
            assert_eq!(result, Err(refusal(why)), "{files:?}");
            assert!(!folder.path().join("out").exists());
        }
        // The pack's own folder written another way, by a second pack.toml.
        let archive = archive::make(&[
            ("ventris-packs-main/top-sfc/en/pack.toml", manifest("top-sfc").as_bytes(), false),
            ("ventris-packs-main/TOP-SFC/fr/pack.toml", manifest("top-sfc").as_bytes(), false),
        ]);
        assert_eq!(unpacked(&archive).1, Err(refusal("top-sfc and TOP-SFC differ only in capital letters")));
    }

    #[test]
    fn an_archive_too_large_or_with_too_many_files_is_refused() {
        // A file that says it unpacks to more than the limit: its size is read from the archive's
        // list, before anything is unpacked.
        let mut large = archive::make(&[("ventris-packs-main/big.txt", b"small", false)]);
        let list = large.len() - 22 - 46 - "ventris-packs-main/big.txt".len();
        large[list + 24..list + 28].copy_from_slice(&((MAX_UNPACKED + 1) as u32).to_le_bytes());
        let (folder, result) = unpacked(&large);
        assert_eq!(result, Err(refusal("it would take 512 MB unpacked, more than the packs can")));
        assert!(!folder.path().join("out").exists());

        let names: Vec<String> = (0..=MAX_ENTRIES).map(|n| format!("ventris-packs-main/{n}.txt")).collect();
        let files: Vec<(&str, &[u8], bool)> = names.iter().map(|name| (name.as_str(), &b""[..], false)).collect();
        let (_, result) = unpacked(&archive::make(&files));
        assert_eq!(result, Err(refusal("it holds 20001 files, more than the 20000 the packs can have")));
    }

    /// What the little server below was asked: the request line and the `If-None-Match` it sent.
    type Asked = Arc<Mutex<Vec<(String, Option<String>)>>>;

    /// One answer of the server below: a status, headers, and a body.
    type Answer = (u16, Vec<(&'static str, String)>, Vec<u8>);

    /// A server on this computer that answers each request with the next of `answers`. It stands
    /// in for GitHub, so that no test uses the network.
    fn serve(answers: Vec<Answer>) -> (String, Asked) {
        serve_held(answers, None)
    }

    /// The same, holding back the second half of the first answer's body until `go` is sent.
    fn serve_held(answers: Vec<Answer>, go: Option<mpsc::Receiver<()>>) -> (String, Asked) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let asked: Asked = Arc::default();
        let log = asked.clone();
        thread::spawn(move || {
            let mut go = go;
            for (status, headers, body) in answers {
                let Ok((stream, _)) = listener.accept() else { return };
                answer(stream, status, &headers, &body, &log, go.take());
            }
        });
        (address, asked)
    }

    fn answer(
        mut stream: TcpStream,
        status: u16,
        headers: &[(&str, String)],
        body: &[u8],
        log: &Asked,
        go: Option<mpsc::Receiver<()>>,
    ) {
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let mut etag = None;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            if header.trim().is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':')
                && name.eq_ignore_ascii_case("if-none-match")
            {
                etag = Some(value.trim().to_string());
            }
        }
        log.lock().unwrap().push((line.trim().to_string(), etag));
        let mut head = format!("HTTP/1.1 {status} Whatever\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
        for (name, value) in headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str("\r\n");
        stream.write_all(head.as_bytes()).unwrap();
        let half = if go.is_some() { body.len() / 2 } else { body.len() };
        stream.write_all(&body[..half]).unwrap();
        stream.flush().unwrap();
        if let Some(go) = go {
            let _ = go.recv();
        }
        stream.write_all(&body[half..]).unwrap();
    }

    fn local(url: String) -> Source {
        Source { url, https_only: false }
    }

    fn strings(store: &Store) -> String {
        fs::read_to_string(store.kept().unwrap().folder.join("top-sfc").join("en").join("01.txt")).unwrap()
    }

    /// The names in the store's folder, sorted, with each copy's name cut to `copy-`.
    fn left(cache: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(cache.join("packs"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .map(|name| if name.starts_with(COPY_PREFIX) { COPY_PREFIX.to_string() } else { name })
            .collect();
        names.sort();
        names
    }

    #[test]
    fn the_packs_are_fetched_kept_and_not_downloaded_again_when_unchanged() {
        let cache = tempfile::tempdir().unwrap();
        let store = Store::new(cache.path());
        assert_eq!(store.kept(), None);
        let tagged = |tag: &str| vec![("ETag", tag.to_string())];
        let (address, asked) = serve(vec![
            // First a redirect, as GitHub sends to codeload, then the archive.
            (302, vec![("Location", "/codeload/main.zip".to_string())], vec![]),
            (200, tagged("\"one\""), packs_archive("1")),
            // The same version again: the tag goes along, through the redirect too.
            (302, vec![("Location", "/codeload/main.zip".to_string())], vec![]),
            (304, tagged("\"one\""), vec![]),
            // A new version, then another.
            (200, tagged("\"two\""), packs_archive("2")),
            (200, tagged("\"three\""), packs_archive("3")),
        ]);
        let source = local(format!("{address}/archive/main.zip"));

        assert_eq!(store.refresh(&source), Ok(Refreshed::Replaced));
        assert_eq!(strings(&store), "[01.000]\nHello, version 1.\n\n");
        let first = store.record().unwrap();
        assert_eq!((first.url.as_str(), first.etag.as_deref()), (source.url.as_str(), Some("\"one\"")));
        assert!(first.checked > 0 && store.kept().unwrap().checked == first.checked);
        let first_folder = store.kept().unwrap().folder;
        assert!(first_folder.file_name().unwrap().to_string_lossy().starts_with(COPY_PREFIX));
        assert_eq!(left(cache.path()), ["copy-", "lock", "record.json"]);

        assert_eq!(store.refresh(&source), Ok(Refreshed::Unchanged));
        assert_eq!(strings(&store), "[01.000]\nHello, version 1.\n\n");
        assert_eq!(store.kept().unwrap().folder, first_folder);

        assert_eq!(store.refresh(&source), Ok(Refreshed::Replaced));
        assert_eq!(strings(&store), "[01.000]\nHello, version 2.\n\n");
        assert_eq!(store.record().unwrap().etag.as_deref(), Some("\"two\""));
        // A new copy is a new folder, and the one before is kept until the next refresh.
        let second_folder = store.kept().unwrap().folder;
        assert!(second_folder != first_folder && first_folder.is_dir());
        assert_eq!(left(cache.path()), ["copy-", "copy-", "lock", "record.json"]);

        assert_eq!(store.refresh(&source), Ok(Refreshed::Replaced));
        assert_eq!(strings(&store), "[01.000]\nHello, version 3.\n\n");
        assert!(!first_folder.exists() && second_folder.is_dir());
        assert_eq!(left(cache.path()), ["copy-", "copy-", "lock", "record.json"]);

        let asked = asked.lock().unwrap();
        let tags: Vec<Option<&str>> = asked.iter().map(|(_, etag)| etag.as_deref()).collect();
        assert_eq!(tags, [None, None, Some("\"one\""), Some("\"one\""), Some("\"one\""), Some("\"two\"")]);
        assert_eq!(asked[1].0, "GET /codeload/main.zip HTTP/1.1");
    }

    #[test]
    fn a_failed_fetch_leaves_the_kept_copy_as_it_was() {
        let cache = tempfile::tempdir().unwrap();
        let store = Store::new(cache.path());
        // A byte of a pack's file changed on the way: its checksum no longer matches.
        let mut damaged = packs_archive("2");
        let notes = damaged.windows(18).position(|bytes| bytes == b"Notes of the pack.").unwrap();
        damaged[notes] ^= 0xFF;
        // A network's sign-in page, in place of the archive.
        let page = b"<!DOCTYPE html><html><body>Sign in to the hotel's Wi-Fi</body></html>".to_vec();
        let (address, _) = serve(vec![
            (200, vec![("ETag", "\"one\"".to_string())], packs_archive("1")),
            // The repository cannot be seen, as while it is private.
            (404, vec![], b"Not Found".to_vec()),
            (200, vec![("ETag", "\"two\"".to_string())], damaged),
            (200, vec![], archive::make(&[("ventris-packs-main/../x.txt", b"x", false)])),
            (200, vec![("Content-Type", "text/html".to_string())], page),
            (200, vec![], b"PK but not an archive at all".to_vec()),
            (200, vec![], pack_with(&[("nul.txt", &b"x"[..])])),
        ]);
        let source = local(format!("{address}/main.zip"));
        let at = host(&source.url).to_string();
        assert_eq!(store.refresh(&source), Ok(Refreshed::Replaced));
        let before = store.record().unwrap();

        for said in [
            format!("{at} answered 404 Not Found"),
            DAMAGED.to_string(),
            refusal("ventris-packs-main/../x.txt would be written outside the folder of the packs"),
            format!(
                "what {at} sent is not an archive of the packs: it may be the sign-in page of a network that asks you \
                 to sign in first, such as a hotel's Wi-Fi"
            ),
            DAMAGED.to_string(),
            refusal(
                "top-sfc/en/nul.txt cannot be unpacked on every system: “nul.txt” is a name Windows keeps for a device",
            ),
        ] {
            assert_eq!(store.refresh(&source), Err(said.clone()));
            // No path on this computer is in what the page is told.
            assert!(!said.contains(&cache.path().display().to_string()) && !said.contains(".part"), "{said}");
            assert_eq!(strings(&store), "[01.000]\nHello, version 1.\n\n");
            assert_eq!(store.record().unwrap(), before);
            // Nothing of the failed refresh is left: no download, no half-written copy.
            assert_eq!(left(cache.path()), ["copy-", "lock", "record.json"], "{said}");
        }

        // With nothing listening at all.
        let closed = TcpListener::bind("127.0.0.1:0").unwrap();
        let nobody = local(format!("http://{}/main.zip", closed.local_addr().unwrap()));
        drop(closed);
        assert_eq!(store.refresh(&nobody), Err(format!("{} could not be reached", host(&nobody.url))));
        assert_eq!(strings(&store), "[01.000]\nHello, version 1.\n\n");
        assert_eq!(left(cache.path()), ["copy-", "lock", "record.json"]);
    }

    #[test]
    fn a_tag_is_only_sent_to_the_address_that_gave_it_and_while_the_copy_is_there() {
        let cache = tempfile::tempdir().unwrap();
        let store = Store::new(cache.path());
        let tagged = vec![("ETag", "\"one\"".to_string())];
        let (address, asked) = serve(vec![
            (200, tagged.clone(), packs_archive("1")),
            (200, tagged.clone(), packs_archive("1")),
            (200, tagged.clone(), packs_archive("1")),
        ]);
        store.refresh(&local(format!("{address}/a.zip"))).unwrap();
        store.refresh(&local(format!("{address}/b.zip"))).unwrap();
        fs::remove_dir_all(store.kept().unwrap().folder).unwrap();
        assert_eq!(store.kept(), None);
        store.refresh(&local(format!("{address}/b.zip"))).unwrap();
        let asked = asked.lock().unwrap();
        assert!(asked.iter().all(|(_, etag)| etag.is_none()), "{asked:?}");
    }

    /// A copy kept by an earlier version of the app, in `current/` with a record that names no
    /// folder, is taken as no copy: the packs are fetched whole, and the old folders go.
    #[test]
    fn a_copy_in_the_old_layout_is_fetched_again_and_removed() {
        let cache = tempfile::tempdir().unwrap();
        let store = Store::new(cache.path());
        let packs = cache.path().join("packs");
        for old in ["current", "incoming", "previous"] {
            fs::create_dir_all(packs.join(old).join("top-sfc").join("en")).unwrap();
            fs::write(packs.join(old).join("top-sfc").join("en").join("pack.toml"), manifest("top-sfc")).unwrap();
        }
        fs::write(packs.join("download.zip.part"), b"half").unwrap();
        let (address, asked) = serve(vec![(200, vec![("ETag", "\"two\"".to_string())], packs_archive("2"))]);
        let source = local(format!("{address}/main.zip"));
        let old = serde_json::json!({ "url": source.url, "etag": "\"one\"", "downloaded": 5, "checked": 6 });
        fs::write(packs.join("record.json"), old.to_string()).unwrap();
        assert_eq!(store.kept(), None);

        assert_eq!(store.refresh(&source), Ok(Refreshed::Replaced));
        assert_eq!(strings(&store), "[01.000]\nHello, version 2.\n\n");
        // The old record's tag is not sent: its copy is not taken as one.
        assert_eq!(asked.lock().unwrap()[0].1, None);
        assert_eq!(left(cache.path()), ["copy-", "lock", "record.json"]);
    }

    /// While a refresh is downloading, the copy there is stays whole and named: a listing made then
    /// reads it, and nothing of the new one. What a refresh that stopped left is never taken as a
    /// copy, and is removed by the next.
    #[test]
    fn a_copy_is_never_seen_half_written() {
        let cache = tempfile::tempdir().unwrap();
        let store = Store::new(cache.path());
        let (address, _) = serve(vec![(200, vec![], packs_archive("1"))]);
        store.refresh(&local(format!("{address}/main.zip"))).unwrap();
        let first = store.kept().unwrap().folder;
        // A refresh that stopped halfway, by another copy of the app.
        let stopped = cache.path().join("packs").join("copy-99-1-0");
        fs::create_dir_all(stopped.join("top-sfc")).unwrap();
        fs::write(cache.path().join("packs").join("download-99-1-0.zip.part"), b"PK half").unwrap();

        let (go, held) = mpsc::channel();
        let (address, _) = serve_held(vec![(200, vec![], packs_archive("2"))], Some(held));
        let source = local(format!("{address}/main.zip"));
        let other = Store::new(cache.path());
        let refreshing = thread::spawn(move || other.refresh(&source));
        // Half the archive has arrived.
        let downloads = || {
            fs::read_dir(cache.path().join("packs"))
                .unwrap()
                .filter(|entry| entry.as_ref().unwrap().file_name().to_string_lossy().starts_with(DOWNLOAD_PREFIX))
                .count()
        };
        let deadline = SystemTime::now() + Duration::from_secs(30);
        while downloads() < 2 {
            assert!(SystemTime::now() < deadline && !refreshing.is_finished(), "the download did not start");
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(store.kept().unwrap().folder, first);
        assert_eq!(strings(&store), "[01.000]\nHello, version 1.\n\n");
        go.send(()).unwrap();
        assert_eq!(refreshing.join().unwrap(), Ok(Refreshed::Replaced));
        assert_eq!(strings(&store), "[01.000]\nHello, version 2.\n\n");
        assert!(!stopped.exists() && first.is_dir());
        assert_eq!(left(cache.path()), ["copy-", "copy-", "lock", "record.json"]);
    }

    /// Two copies of the app: while one holds the lock, the other downloads nothing, waits, and
    /// then has the copy the first put in place.
    #[test]
    fn while_another_copy_of_the_app_refreshes_this_one_waits_and_fetches_nothing() {
        let cache = tempfile::tempdir().unwrap();
        let store = Store::new(cache.path());
        fs::create_dir_all(cache.path().join("packs")).unwrap();
        // The other copy of the app, holding the lock as its refresh does.
        let lock = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(cache.path().join("packs").join(LOCK_FILE))
            .unwrap();
        lock.lock().unwrap();
        let (address, asked) = serve(vec![(200, vec![], packs_archive("1"))]);
        let source = local(format!("{address}/main.zip"));
        let waiting = {
            let store = Store::new(cache.path());
            let source = source.clone();
            thread::spawn(move || store.refresh(&source))
        };
        thread::sleep(Duration::from_millis(300));
        assert!(!waiting.is_finished() && asked.lock().unwrap().is_empty());
        // What the other puts in place meanwhile.
        let folder = cache.path().join("packs").join("copy-other");
        let archive = cache.path().join("other.zip");
        fs::write(&archive, packs_archive("from the other")).unwrap();
        unpack(&archive, &folder).unwrap();
        store.adopt("copy-other", 7);
        drop(lock);

        assert_eq!(waiting.join().unwrap(), Ok(Refreshed::ByAnother));
        assert!(asked.lock().unwrap().is_empty());
        assert_eq!(store.kept(), Some(Kept { folder, checked: 7 }));
        assert_eq!(strings(&store), "[01.000]\nHello, version from the other.\n\n");
    }

    /// Two refreshes at once in this app: one downloads, and the other, after it, finds the copy up
    /// to date. One whole copy is left.
    #[test]
    fn two_refreshes_at_once_leave_one_whole_copy() {
        let cache = tempfile::tempdir().unwrap();
        let tagged = vec![("ETag", "\"one\"".to_string())];
        let (address, asked) = serve(vec![(200, tagged.clone(), packs_archive("1")), (304, tagged, vec![])]);
        let source = local(format!("{address}/main.zip"));
        let refreshes: Vec<_> = (0..2)
            .map(|_| {
                let (store, source) = (Store::new(cache.path()), source.clone());
                thread::spawn(move || store.refresh(&source))
            })
            .collect();
        let mut done: Vec<Refreshed> = refreshes.into_iter().map(|refresh| refresh.join().unwrap().unwrap()).collect();
        done.sort_by_key(|refreshed| format!("{refreshed:?}"));
        assert_eq!(done, [Refreshed::Replaced, Refreshed::Unchanged]);
        assert_eq!(asked.lock().unwrap().len(), 2);
        assert_eq!(strings(&Store::new(cache.path())), "[01.000]\nHello, version 1.\n\n");
        assert_eq!(left(cache.path()), ["copy-", "lock", "record.json"]);
    }

    /// On Windows a folder with a file open in it cannot be renamed or removed. A refresh then still
    /// puts its copy in place, and the old folder goes at a later refresh, once the file is closed.
    #[cfg(windows)]
    #[test]
    fn a_file_open_in_the_current_copy_does_not_stop_a_refresh() {
        use std::os::windows::fs::OpenOptionsExt;

        let cache = tempfile::tempdir().unwrap();
        let store = Store::new(cache.path());
        let (address, _) = serve(vec![
            (200, vec![], packs_archive("1")),
            (200, vec![], packs_archive("2")),
            (200, vec![], packs_archive("3")),
            (200, vec![], packs_archive("4")),
        ]);
        let source = local(format!("{address}/main.zip"));
        store.refresh(&source).unwrap();
        let first = store.kept().unwrap().folder;
        // Opened as an editor or a virus scanner may: others may read it, but not delete it.
        let open =
            File::options().read(true).share_mode(1).open(first.join("top-sfc").join("en").join("01.txt")).unwrap();
        assert!(fs::rename(&first, cache.path().join("moved")).is_err());

        assert_eq!(store.refresh(&source), Ok(Refreshed::Replaced));
        assert_eq!(strings(&store), "[01.000]\nHello, version 2.\n\n");
        assert_eq!(store.refresh(&source), Ok(Refreshed::Replaced));
        assert_eq!(strings(&store), "[01.000]\nHello, version 3.\n\n");
        // Two copies back, but its file is open: it is left.
        assert!(first.join("top-sfc").join("en").join("01.txt").is_file());
        drop(open);
        assert_eq!(store.refresh(&source), Ok(Refreshed::Replaced));
        assert_eq!(strings(&store), "[01.000]\nHello, version 4.\n\n");
        assert!(!first.exists());
        assert_eq!(left(cache.path()), ["copy-", "copy-", "lock", "record.json"]);
    }

    /// A record edited to name a folder elsewhere is not followed.
    #[test]
    fn a_record_names_a_folder_beside_it_and_nowhere_else() {
        let cache = tempfile::tempdir().unwrap();
        let store = Store::new(cache.path());
        fs::create_dir_all(cache.path().join("elsewhere")).unwrap();
        fs::create_dir_all(cache.path().join("packs").join("copy-")).unwrap();
        for name in ["../elsewhere", "copy-/../../elsewhere", "copy-", "current", "C:\\elsewhere"] {
            store.adopt(name, 1);
            assert_eq!(store.kept(), None, "{name}");
        }
        fs::create_dir_all(cache.path().join("packs").join("copy-1")).unwrap();
        store.adopt("copy-1", 1);
        assert_eq!(store.kept().unwrap().folder, cache.path().join("packs").join("copy-1"));
    }
}
