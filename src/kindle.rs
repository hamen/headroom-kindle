//! Pushes the dashboard to the Kindle over the USBNetwork SSH link and draws it with `eips`.
//! See KINDLE_HACKS.md for why the Amazon UI is stopped and why it has to be `eips`.

use ssh2::{HashType, Session};
use std::fmt::Display;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

/// The Kindle on the USB network link (the "Kindle USB" NetworkManager profile on the PC).
const ADDRESS: &str = "192.168.15.244:22";
const USER: &str = "root";
/// Where the root password lives, outside the repo (relative to $HOME).
const PASSWORD_FILE: &str = ".config/headroom-kindle/kindle-password";
/// The SHA-256 fingerprint of the Kindle's SSH host key, pinned on the first run (relative to $HOME).
const HOST_KEY_FILE: &str = ".config/headroom-kindle/kindle-host-key";
const REMOTE_IMAGE: &str = "/mnt/us/dashboard.png";

/// The shell command run on the Kindle. It first saves the PNG it reads on stdin: plain `cat`,
/// because libssh2's SFTP upload fails against the Kindle's SFTP server. `stop lab126_gui` keeps
/// the Amazon UI from painting over the dashboard; once the UI is stopped it does nothing. A full
/// refresh clears the screen twice to remove ghosting, which flashes it black; a plain one
/// redraws the image in place.
pub fn remote_command(full: bool) -> String {
    let draw = if full {
        format!(
            "/usr/sbin/eips -c; /usr/sbin/eips -c; sleep 1; /usr/sbin/eips -f -g {REMOTE_IMAGE}"
        )
    } else {
        format!("/usr/sbin/eips -g {REMOTE_IMAGE}")
    };
    format!("cat > {REMOTE_IMAGE} || exit 1; stop lab126_gui 2>/dev/null; {draw}")
}

/// `KINDLE_PASSWORD` wins; otherwise the first line of the password file.
pub fn password(env: Option<String>, file: Option<String>) -> Result<String, String> {
    env.or(file)
        .and_then(|p| p.lines().next().map(str::to_string))
        .filter(|p| !p.is_empty())
        .ok_or_else(|| {
            format!("no Kindle password: set KINDLE_PASSWORD or write ~/{PASSWORD_FILE}")
        })
}

fn read_password() -> Result<String, String> {
    let file = std::env::var_os("HOME")
        .and_then(|home| std::fs::read_to_string(PathBuf::from(home).join(PASSWORD_FILE)).ok());
    password(std::env::var("KINDLE_PASSWORD").ok(), file)
}

#[derive(Debug, PartialEq)]
pub enum HostKey {
    /// The key matches the pinned one.
    Known,
    /// Nothing is pinned yet: pin this key.
    New,
}

/// Compares the Kindle's host key fingerprint with the pinned one. A different key is an error, so
/// the root password never goes to another machine that answers at the Kindle's address.
pub fn check_host_key(pinned: Option<&str>, actual: &str) -> Result<HostKey, String> {
    match pinned.map(str::trim).filter(|p| !p.is_empty()) {
        None => Ok(HostKey::New),
        Some(p) if p == actual => Ok(HostKey::Known),
        Some(p) => Err(format!(
            "the Kindle's SSH host key changed (pinned {p}, got {actual}), no password sent. \
             If the Kindle was reset or jailbroken again, delete ~/{HOST_KEY_FILE}"
        )),
    }
}

fn verify_host_key(session: &Session) -> Result<(), String> {
    let hash = session
        .host_key_hash(HashType::Sha256)
        .ok_or("ssh handshake: no host key")?;
    let actual: String = hash.iter().map(|b| format!("{b:02x}")).collect();
    let path = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(HOST_KEY_FILE))
        .ok_or("HOME is not set")?;
    let pinned = std::fs::read_to_string(&path).ok();
    if check_host_key(pinned.as_deref(), &actual)? == HostKey::New {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(&path, format!("{actual}\n"))
            .map_err(|e| format!("cannot pin the host key in {}: {e}", path.display()))?;
        eprintln!("kindle: pinned the SSH host key sha256:{actual}");
    }
    Ok(())
}

/// Uploads the PNG and draws it.
pub fn push(png: &[u8], full: bool) -> Result<(), String> {
    let password = read_password()?;
    let address: SocketAddr = ADDRESS.parse().expect("valid Kindle address");
    let tcp = TcpStream::connect_timeout(&address, Duration::from_secs(10))
        .map_err(|e| format!("cannot reach the Kindle at {ADDRESS}: {e}"))?;
    let mut session = Session::new().map_err(step("ssh session"))?;
    session.set_tcp_stream(tcp);
    session.set_timeout(30_000);
    session.handshake().map_err(step("ssh handshake"))?;
    verify_host_key(&session)?;
    session
        .userauth_password(USER, &password)
        .map_err(step("ssh login"))?;

    let mut channel = session.channel_session().map_err(step("ssh channel"))?;
    channel
        .exec(&remote_command(full))
        .map_err(step("ssh exec"))?;
    channel.write_all(png).map_err(step("upload"))?;
    channel.send_eof().map_err(step("upload"))?;
    let mut output = String::new();
    channel.read_to_string(&mut output).map_err(step("eips"))?;
    let mut errors = String::new();
    channel
        .stderr()
        .read_to_string(&mut errors)
        .map_err(step("eips"))?;
    channel.wait_close().map_err(step("eips"))?;
    match channel.exit_status().map_err(step("eips"))? {
        0 => Ok(()),
        status => Err(format!("Kindle command exited {status}: {}", errors.trim())),
    }
}

fn step<E: Display>(name: &'static str) -> impl Fn(E) -> String {
    move |e| format!("{name}: {e}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_refresh_saves_then_clears_before_drawing() {
        let full = remote_command(true);
        assert!(full.starts_with("cat > /mnt/us/dashboard.png || exit 1; stop lab126_gui"));
        assert_eq!(full.matches("eips -c").count(), 2);
        assert!(full.ends_with("eips -f -g /mnt/us/dashboard.png"));
    }

    #[test]
    fn plain_refresh_only_redraws() {
        let plain = remote_command(false);
        assert!(plain.starts_with("cat > /mnt/us/dashboard.png || exit 1; "));
        assert!(!plain.contains("eips -c"));
        assert!(plain.ends_with("eips -g /mnt/us/dashboard.png"));
    }

    #[test]
    fn password_from_env_or_file() {
        let env = Some("fake-env".to_string());
        let file = Some("fake-file\n".to_string());
        assert_eq!(password(env, file.clone()).unwrap(), "fake-env");
        assert_eq!(password(None, file).unwrap(), "fake-file");
        assert!(password(None, None).is_err());
        assert!(password(None, Some("\n".to_string())).is_err());
    }

    #[test]
    fn host_key_is_pinned_then_enforced() {
        assert_eq!(check_host_key(None, "ab12").unwrap(), HostKey::New);
        assert_eq!(check_host_key(Some("\n"), "ab12").unwrap(), HostKey::New);
        assert_eq!(
            check_host_key(Some("ab12\n"), "ab12").unwrap(),
            HostKey::Known
        );
        let changed = check_host_key(Some("ab12"), "cd34").unwrap_err();
        assert!(changed.contains("changed") && changed.contains("no password sent"));
    }
}
