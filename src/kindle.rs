//! Pushes the dashboard to the Kindle over the USBNetwork SSH link and draws it with `eips`.
//! See KINDLE_HACKS.md for why the Amazon UI is stopped and why it has to be `eips`.

use ssh2::Session;
use std::fmt::Display;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The Kindle on the USB network link (the "Kindle USB" NetworkManager profile on the PC).
const ADDRESS: &str = "192.168.15.244:22";
const USER: &str = "root";
/// Where the root password lives, outside the repo (relative to $HOME).
const PASSWORD_FILE: &str = ".config/headroom-kindle/kindle-password";
const REMOTE_IMAGE: &str = "/mnt/us/dashboard.png";

/// The shell command run on the Kindle. `stop lab126_gui` keeps the Amazon UI from painting over
/// the dashboard; once the UI is stopped it does nothing. A full refresh clears the screen twice
/// to remove ghosting, which flashes it black; a plain one redraws the image in place.
pub fn draw_command(full: bool) -> String {
    if full {
        format!(
            "stop lab126_gui 2>/dev/null; /usr/sbin/eips -c; /usr/sbin/eips -c; sleep 1; \
             /usr/sbin/eips -f -g {REMOTE_IMAGE}"
        )
    } else {
        format!("stop lab126_gui 2>/dev/null; /usr/sbin/eips -g {REMOTE_IMAGE}")
    }
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
    session
        .userauth_password(USER, &password)
        .map_err(step("ssh login"))?;

    let sftp = session.sftp().map_err(step("sftp"))?;
    let mut file = sftp
        .create(Path::new(REMOTE_IMAGE))
        .map_err(step("upload"))?;
    file.write_all(png).map_err(step("upload"))?;
    drop(file);

    let mut channel = session.channel_session().map_err(step("ssh channel"))?;
    channel.exec(&draw_command(full)).map_err(step("eips"))?;
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
        status => Err(format!("eips exited {status}: {}", errors.trim())),
    }
}

fn step<E: Display>(name: &'static str) -> impl Fn(E) -> String {
    move |e| format!("{name}: {e}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_refresh_clears_before_drawing() {
        let full = draw_command(true);
        assert!(full.starts_with("stop lab126_gui"));
        assert_eq!(full.matches("eips -c").count(), 2);
        assert!(full.ends_with("eips -f -g /mnt/us/dashboard.png"));
    }

    #[test]
    fn plain_refresh_only_redraws() {
        let plain = draw_command(false);
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
}
