//! The dev server can approve and run Python, so a page from another site must not be able to call it, in a browser or by DNS rebinding.
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};

struct Server(Child, u16, tempfile::TempDir);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start() -> Server {
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let data = tempfile::tempdir().unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_devserver"))
        .env("GIS_DEVSERVER_ADDR", format!("127.0.0.1:{port}"))
        .env("GIS_STUDIO_DATA", data.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    for _ in 0..200 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Server(child, port, data);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("the dev server did not start");
}

/// (status line, lowercase header block, body) of a raw request.
fn request(port: u16, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> (String, String, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let host = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("host")).map_or(format!("127.0.0.1:{port}"), |(_, v)| v.to_string());
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Length: {}\r\n", body.len());
    for (k, v) in headers.iter().filter(|(k, _)| !k.eq_ignore_ascii_case("host")) {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(body);
    s.write_all(req.as_bytes()).unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    let (head, body) = out.split_once("\r\n\r\n").unwrap_or((&out, ""));
    let (status, headers) = head.split_once("\r\n").unwrap_or((head, ""));
    (status.to_string(), headers.to_lowercase(), body.to_string())
}

#[test]
fn a_page_from_another_site_cannot_approve_or_run_anything() {
    let s = start();
    let script = "def calculate(row):\n    return True\n";
    let body = serde_json::json!({ "script": script }).to_string();
    // A foreign Origin is refused outright (the request is not carried out, not merely hidden from the page)...
    for origin in ["https://evil.example", "http://localhost.evil.example", "null", "http://127.0.0.1.evil.example"] {
        let (status, headers, _) = request(s.1, "POST", "/invoke/approve_script", &[("Origin", origin), ("Content-Type", "text/plain")], &body);
        assert!(status.contains("403"), "{origin}: {status}");
        assert!(!headers.contains("access-control-allow-origin"), "{origin}: {headers}");
    }
    // ...so the script is still not approved.
    let (status, _, out) = request(s.1, "POST", "/invoke/script_trust_info", &[("Content-Type", "application/json")], &body);
    assert!(status.contains("200"), "{status}");
    assert!(out.contains("\"approved\":false"), "{out}");
    // A rebinding page arrives under its own host name.
    let (status, _, _) = request(s.1, "POST", "/invoke/approve_script", &[("Host", "evil.example:7878")], &body);
    assert!(status.contains("403"), "{status}");
    // Preflight from a foreign origin is refused too; from the local UI it echoes that origin, never `*`.
    let (status, headers, _) = request(s.1, "OPTIONS", "/invoke/approve_script", &[("Origin", "https://evil.example"), ("Access-Control-Request-Method", "POST")], "");
    assert!(status.contains("403") && !headers.contains("access-control-allow-origin"), "{status} {headers}");
    let (status, headers, _) = request(s.1, "OPTIONS", "/invoke/approve_script", &[("Origin", "http://localhost:1420"), ("Access-Control-Request-Method", "POST")], "");
    assert!(status.contains("204"), "{status}");
    assert!(headers.contains("access-control-allow-origin: http://localhost:1420") && !headers.contains("access-control-allow-origin: *"), "{headers}");
}

#[test]
fn the_local_ui_curl_and_the_e2e_runner_keep_working() {
    let s = start();
    let body = serde_json::json!({ "script": "def calculate(row):\n    return True\n" }).to_string();
    // No Origin (curl, Playwright's request API): works, and no CORS header is added.
    let (status, headers, out) = request(s.1, "POST", "/invoke/script_trust_info", &[("Content-Type", "application/json")], &body);
    assert!(status.contains("200") && out.contains("\"approved\":false") && !headers.contains("access-control-allow-origin"), "{status} {headers} {out}");
    // The Vite page on localhost: allowed, echoed exactly.
    let (status, headers, _) = request(s.1, "POST", "/invoke/script_trust_info", &[("Origin", "http://localhost:1420"), ("Content-Type", "application/json")], &body);
    assert!(status.contains("200") && headers.contains("access-control-allow-origin: http://localhost:1420"), "{status} {headers}");
    let (status, headers, _) = request(s.1, "POST", "/invoke/script_trust_info", &[("Origin", "http://127.0.0.1:1420")], &body);
    assert!(status.contains("200") && headers.contains("access-control-allow-origin: http://127.0.0.1:1420"), "{status} {headers}");
}
