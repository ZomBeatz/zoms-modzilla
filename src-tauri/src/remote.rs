//! Send from your phone: a small page on this computer, reached on the home network by
//! scanning a QR code, for pasting what's awkward to type on a Steam Deck: your Nexus Mods API
//! key, a Mod manager download link (nxm://, which carries the key and expiry Nexus Mods made
//! when you clicked, so free accounts still click each file), or a mod or collection address.
//!
//! It only listens while its dialog is open (and 15 minutes at most), every request needs the
//! one-time code in the QR code, and it only takes those three things.

use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

pub const OPEN_FOR: Duration = Duration::from_secs(15 * 60);

/// What came in from the phone.
#[derive(Debug, Clone, PartialEq)]
pub enum Sent {
    Key(String),
    Link(String),
}

pub struct Server {
    stop: Arc<AtomicBool>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub static RUNNING: Mutex<Option<Server>> = Mutex::new(None);

/// This computer's address on the home network (no packet is sent to find it).
pub fn lan_ip() -> Option<std::net::IpAddr> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

pub fn token() -> String {
    let mut b = [0u8; 16];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        let _ = f.read_exact(&mut b);
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// What a link from the phone may be.
pub fn link_ok(s: &str) -> bool {
    let s = s.trim();
    (s.starts_with("nxm://") || s.starts_with("https://www.nexusmods.com/") || s.starts_with("https://next.nexusmods.com/") || s.starts_with("https://nexusmods.com/")) && s.len() < 2048 && !s.chars().any(char::is_whitespace)
}

fn form_value(body: &str, name: &str) -> Option<String> {
    body.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == name).then(|| url::form_urlencoded::parse(format!("x={v}").as_bytes()).next().map(|(_, v)| v.into_owned()).unwrap_or_default())
    })
}

/// Read one request: (method, path with query, body). Small requests only.
fn read_request(s: &mut TcpStream) -> Option<(String, String, String)> {
    s.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut buf = vec![];
    let mut chunk = [0u8; 4096];
    loop {
        let n = s.read(&mut chunk).ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() > 16 * 1024 {
            return None;
        }
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..end]).into_owned();
            let len: usize = head.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0))).unwrap_or(0);
            if len > 8 * 1024 {
                return None;
            }
            while buf.len() < end + 4 + len {
                let n = s.read(&mut chunk).ok()?;
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            let first = head.lines().next()?.to_string();
            let mut parts = first.split(' ');
            let (m, p) = (parts.next()?.to_string(), parts.next()?.to_string());
            let body = String::from_utf8_lossy(&buf[end + 4..(end + 4 + len).min(buf.len())]).into_owned();
            return Some((m, p, body));
        }
    }
    None
}

fn reply(s: &mut TcpStream, status: &str, kind: &str, body: &str) {
    let _ = write!(
        s,
        "HTTP/1.1 {status}\r\nContent-Type: {kind}; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; form-action 'self'\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

pub fn page(token: &str, said: &str) -> String {
    let said = said.replace('&', "&amp;").replace('<', "&lt;");
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Send to Zom's Nexus</title>
<style>body{{font:16px system-ui,sans-serif;background:#0d0d12;color:#ececf1;margin:0;padding:20px;max-width:560px}}h1{{font-size:20px}}p{{color:#a7a7b5}}label{{display:block;margin:18px 0 6px;font-weight:600}}textarea,input{{width:100%;box-sizing:border-box;padding:12px;border-radius:10px;border:1px solid #33333f;background:#17171f;color:inherit;font:inherit}}button{{margin-top:10px;padding:12px 18px;border:0;border-radius:10px;background:#7c5cff;color:#fff;font:inherit;font-weight:600}}.said{{padding:12px;border-radius:10px;background:#1d2a1f;color:#b9f5c4}}a{{color:#b6a6ff}}</style></head><body>
<h1>Send to Zom's Nexus</h1>{said_block}
<form method="post" action="/link?t={token}"><label for="l">A link</label><p>A <b>Mod manager download</b> link (long-press the button on nexusmods.com, copy the link), a mod page, or a collection.</p><textarea id="l" name="link" rows="3" placeholder="nxm://… or https://www.nexusmods.com/…"></textarea><button>Send</button></form>
<form method="post" action="/key?t={token}"><label for="k">Your API key</label><p>Copy it from <a href="https://next.nexusmods.com/settings/api-keys" rel="noreferrer">nexusmods.com › Settings › API keys</a>.</p><input id="k" name="key" type="password" autocomplete="off"><button>Sign in</button></form>
<p>This page works only while Send from your phone is open in Zom's Nexus. Use it on your home network.</p></body></html>"#,
        said_block = if said.is_empty() { String::new() } else { format!(r#"<p class="said">{said}</p>"#) }
    )
}

/// Start listening (stopping any earlier one). `got` is called for each thing sent, and says
/// what to show on the phone.
pub fn start(got: impl Fn(Sent) -> String + Send + 'static) -> Result<String, String> {
    let ip = lan_ip().ok_or("This computer isn't on a network.")?;
    let l = TcpListener::bind((ip, 0)).map_err(|e| format!("Couldn't open the page: {e}"))?;
    l.set_nonblocking(true).map_err(|e| e.to_string())?;
    let port = l.local_addr().map_err(|e| e.to_string())?.port();
    let tok = token();
    let url = format!("http://{}:{port}/?t={tok}", if ip.is_ipv6() { format!("[{ip}]") } else { ip.to_string() });
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    std::thread::spawn(move || {
        let began = Instant::now();
        while !stop2.load(Ordering::Relaxed) && began.elapsed() < OPEN_FOR {
            let Ok((mut s, _)) = l.accept() else {
                std::thread::sleep(Duration::from_millis(150));
                continue;
            };
            let _ = s.set_nonblocking(false);
            let Some((method, path, body)) = read_request(&mut s) else { continue };
            let (route, query) = path.split_once('?').unwrap_or((path.as_str(), ""));
            let t = form_value(query, "t").unwrap_or_default();
            if !same(&t, &tok) {
                reply(&mut s, "403 Forbidden", "text/plain", "This page's code doesn't match. Scan the QR code in Zom's Nexus again.");
                continue;
            }
            let said = match (method.as_str(), route) {
                ("GET", "/") => String::new(),
                ("POST", "/link") => match form_value(&body, "link").map(|v| v.trim().to_string()) {
                    Some(v) if link_ok(&v) => got(Sent::Link(v)),
                    _ => "That isn't a Nexus Mods link.".into(),
                },
                ("POST", "/key") => match form_value(&body, "key").map(|v| v.trim().to_string()) {
                    Some(v) if !v.is_empty() && v.len() < 512 && v.chars().all(|c| c.is_ascii_graphic()) => got(Sent::Key(v)),
                    _ => "Paste the whole key.".into(),
                },
                _ => {
                    reply(&mut s, "404 Not Found", "text/plain", "Not here.");
                    continue;
                }
            };
            reply(&mut s, "200 OK", "text/html", &page(&tok, &said));
        }
    });
    *RUNNING.lock().unwrap() = Some(Server { stop });
    Ok(url)
}

pub fn stop() {
    *RUNNING.lock().unwrap() = None;
}

/// A QR code's squares, row by row (true = dark), and its width.
pub fn qr(text: &str) -> Result<(usize, Vec<bool>), String> {
    let c = qrcode::QrCode::new(text.as_bytes()).map_err(|e| e.to_string())?;
    Ok((c.width(), c.to_colors().into_iter().map(|x| x == qrcode::Color::Dark).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_takes_nexus_links_and_the_code() {
        assert!(link_ok("nxm://skyrimspecialedition/mods/1/files/2?key=a&expires=1"));
        assert!(link_ok("https://www.nexusmods.com/games/stardewvalley/collections/abc"));
        assert!(!link_ok("https://evil.example/nxm://"));
        assert!(!link_ok("javascript:alert(1)"));
        assert_eq!(form_value("link=nxm%3A%2F%2Fa%2Fb&x=1", "link").as_deref(), Some("nxm://a/b"));
        assert_eq!(form_value("key=a+b", "key").as_deref(), Some("a b"));
        assert!(same("abc", "abc") && !same("abc", "abd") && !same("abc", "ab"));
        assert_eq!(token().len(), 32);
        assert!(page("t", "<b>").contains("&lt;b>"));
        let (w, cells) = qr("http://192.168.1.5:4242/?t=00").unwrap();
        assert_eq!(cells.len(), w * w);
    }

    #[test]
    fn serves_the_page_only_with_the_code() {
        if lan_ip().is_none() {
            return;
        }
        let got = Arc::new(Mutex::new(vec![]));
        let g2 = got.clone();
        let url = start(move |s| {
            g2.lock().unwrap().push(s);
            "Sent.".into()
        })
        .unwrap();
        let rest = url.strip_prefix("http://").unwrap();
        let (host, q) = rest.split_once('/').unwrap();
        let ask = |req: String| {
            let mut s = TcpStream::connect(host).unwrap();
            s.write_all(req.as_bytes()).unwrap();
            let mut out = String::new();
            let _ = s.read_to_string(&mut out);
            out
        };
        assert!(ask(format!("GET /{q} HTTP/1.1\r\nHost: x\r\n\r\n")).contains("Send to Zom's Nexus"));
        assert!(ask("GET /?t=wrong HTTP/1.1\r\n\r\n".into()).starts_with("HTTP/1.1 403"));
        let body = "link=nxm%3A%2F%2Fstardewvalley%2Fmods%2F1%2Ffiles%2F2";
        let r = ask(format!("POST /link{} HTTP/1.1\r\nContent-Length: {}\r\n\r\n{body}", &q[0..].trim_start_matches(|c| c != '?'), body.len()));
        assert!(r.contains("Sent."));
        assert_eq!(got.lock().unwrap()[0], Sent::Link("nxm://stardewvalley/mods/1/files/2".into()));
        stop();
    }
}

