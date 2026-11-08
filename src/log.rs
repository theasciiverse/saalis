//! Session logging, per the Asciiverse convention: every raw input event,
//! every state change and a periodic snapshot, written to a file so a bug
//! report can carry it. Torched's pattern: the log is
//! `$XDG_STATE_HOME/saalis/session.log` (or `~/.local/state/saalis/`, or
//! `%LOCALAPPDATA%\saalis\` on Windows), started fresh each run. The level is
//! debug unless `SAALIS_LOG` says otherwise (e.g. `SAALIS_LOG=info`).

use std::io::Write;
use std::path::PathBuf;

fn dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("SAALIS_LOG_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    if cfg!(windows) {
        return std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("saalis"));
    }
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .map(|d| d.join("saalis"))
}

/// Starts the log; returns where it is written, if anywhere.
pub fn init() -> Option<PathBuf> {
    let dir = dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("session.log");
    let file = std::fs::File::create(&path).ok()?;
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Debug)
        // the audio stack is chatty below warnings
        .filter_module("symphonia", log::LevelFilter::Warn)
        .filter_module("gilrs", log::LevelFilter::Info)
        .parse_env("SAALIS_LOG")
        .target(env_logger::Target::Pipe(Box::new(file)))
        .format(|buf, rec| writeln!(buf, "{} {:<5} {}", buf.timestamp_millis(), rec.level(), rec.args()))
        .init();
    Some(path)
}
