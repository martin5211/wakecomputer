use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::OnceLock;

static LISTEN: OnceLock<String> = OnceLock::new();
static TOKEN: OnceLock<String> = OnceLock::new();

fn main() {
    let mut listen = "0.0.0.0:9877".to_string();
    let mut token = String::new();

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--listen" => {
                i += 1;
                listen = args[i].clone();
            }
            "--token" => {
                i += 1;
                token = args[i].clone();
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    if token.is_empty() {
        eprintln!("usage: wakecomputer-agent --token <token> [--listen <addr:port>]");
        std::process::exit(1);
    }

    LISTEN.set(listen.clone()).unwrap();
    TOKEN.set(token.clone()).unwrap();

    #[cfg(windows)]
    {
        // Try to start as a Windows service. If SCM is not the caller
        // (i.e. running from a console), this returns false and we
        // fall through to run_server() directly.
        if win_service::start_as_service() {
            return;
        }
    }

    run_server(&listen, &token);
}

fn run_server(listen: &str, token: &str) {
    let listener = TcpListener::bind(listen).unwrap_or_else(|e| {
        eprintln!("failed to bind {listen}: {e}");
        std::process::exit(1);
    });

    eprintln!("listening on {listen}");

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("accept error: {e}");
                continue;
            }
        };

        let mut buf = [0u8; 4096];
        let n = match stream.read(&mut buf) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("read error: {e}");
                continue;
            }
        };

        let request = String::from_utf8_lossy(&buf[..n]);

        let first_line = request.lines().next().unwrap_or("");
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        if parts.len() < 2 {
            let _ = respond(&mut stream, 400, r#"{"error":"bad request"}"#);
            continue;
        }

        let method = parts[0];
        let path = parts[1];

        match (method, path) {
            ("GET", "/health") => {
                let _ = respond(&mut stream, 200, r#"{"status":"ok"}"#);
            }
            ("POST", "/shutdown") => {
                // Check bearer token
                let authorized = request
                    .lines()
                    .find(|line| line.to_ascii_lowercase().starts_with("authorization:"))
                    .and_then(|line| line.splitn(2, ':').nth(1))
                    .map(|val| val.trim())
                    .and_then(|val| val.strip_prefix("Bearer "))
                    .is_some_and(|t| t == token);

                if !authorized {
                    let _ = respond(&mut stream, 401, r#"{"error":"unauthorized"}"#);
                    continue;
                }

                let _ = respond(&mut stream, 200, r#"{"status":"ok"}"#);
                drop(stream);

                eprintln!("shutdown requested, executing...");
                do_shutdown();
            }
            _ => {
                let _ = respond(&mut stream, 404, r#"{"error":"not found"}"#);
            }
        }
    }
}

fn respond(stream: &mut impl Write, status: u16, body: &str) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

fn do_shutdown() {
    let result = if cfg!(target_os = "windows") {
        Command::new("shutdown").args(["/s", "/t", "1"]).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("shutdown").args(["-h", "now"]).spawn()
    } else {
        Command::new("shutdown").arg("now").spawn()
    };

    if let Err(e) = result {
        eprintln!("failed to execute shutdown: {e}");
    }
}

// ---------------------------------------------------------------------------
// Windows service support — raw Win32 FFI, no external crates
// ---------------------------------------------------------------------------
#[cfg(windows)]
mod win_service {
    use super::{LISTEN, TOKEN, run_server};
    use std::ptr;

    const SERVICE_WIN32_OWN_PROCESS: u32 = 0x10;
    const SERVICE_START_PENDING: u32 = 0x02;
    const SERVICE_RUNNING: u32 = 0x04;
    const SERVICE_STOPPED: u32 = 0x01;
    const SERVICE_ACCEPT_STOP: u32 = 0x01;
    const SERVICE_CONTROL_STOP: u32 = 0x01;
    const ERROR_FAILED_SERVICE_CONTROLLER_CONNECT: u32 = 1063;

    #[allow(non_camel_case_types)]
    type SERVICE_STATUS_HANDLE = isize;

    #[repr(C)]
    struct SERVICE_TABLE_ENTRYW {
        service_name: *const u16,
        service_proc: Option<unsafe extern "system" fn(u32, *mut *mut u16)>,
    }

    #[repr(C)]
    struct SERVICE_STATUS {
        service_type: u32,
        current_state: u32,
        controls_accepted: u32,
        win32_exit_code: u32,
        service_specific_exit_code: u32,
        check_point: u32,
        wait_hint: u32,
    }

    unsafe impl Send for SERVICE_TABLE_ENTRYW {}
    unsafe impl Sync for SERVICE_TABLE_ENTRYW {}

    #[link(name = "advapi32")]
    unsafe extern "system" {
        safe fn StartServiceCtrlDispatcherW(table: *const SERVICE_TABLE_ENTRYW) -> i32;
        safe fn RegisterServiceCtrlHandlerW(
            name: *const u16,
            handler: unsafe extern "system" fn(u32),
        ) -> SERVICE_STATUS_HANDLE;
        safe fn SetServiceStatus(
            handle: SERVICE_STATUS_HANDLE,
            status: *mut SERVICE_STATUS,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        safe fn GetLastError() -> u32;
    }

    static mut STATUS_HANDLE: SERVICE_STATUS_HANDLE = 0;

    /// Try to register with the Windows Service Control Manager.
    /// Returns `true` if we started as a service (blocks until service stops).
    /// Returns `false` if called from a console (not launched by SCM).
    pub fn start_as_service() -> bool {
        let name: Vec<u16> = "wakecomputer-agent\0".encode_utf16().collect();

        let table = [
            SERVICE_TABLE_ENTRYW {
                service_name: name.as_ptr(),
                service_proc: Some(service_main),
            },
            SERVICE_TABLE_ENTRYW {
                service_name: ptr::null(),
                service_proc: None,
            },
        ];

        let ok = StartServiceCtrlDispatcherW(table.as_ptr());
        if ok != 0 {
            return true; // ran as service, now finished
        }

        let err = GetLastError();
        if err == ERROR_FAILED_SERVICE_CONTROLLER_CONNECT {
            return false; // not launched by SCM — run as console
        }

        eprintln!("StartServiceCtrlDispatcher failed: error {err}");
        std::process::exit(1);
    }

    unsafe extern "system" fn service_main(_argc: u32, _argv: *mut *mut u16) {
        let name: Vec<u16> = "wakecomputer-agent\0".encode_utf16().collect();

        unsafe {
            STATUS_HANDLE = RegisterServiceCtrlHandlerW(name.as_ptr(), service_control_handler);
        }
        if unsafe { STATUS_HANDLE } == 0 {
            return;
        }

        set_status(SERVICE_START_PENDING, 0);
        set_status(SERVICE_RUNNING, SERVICE_ACCEPT_STOP);

        let listen = LISTEN.get().unwrap();
        let token = TOKEN.get().unwrap();
        run_server(listen, token);

        set_status(SERVICE_STOPPED, 0);
    }

    unsafe extern "system" fn service_control_handler(control: u32) {
        if control == SERVICE_CONTROL_STOP {
            set_status(SERVICE_STOPPED, 0);
            std::process::exit(0);
        }
    }

    fn set_status(state: u32, controls: u32) {
        let mut status = SERVICE_STATUS {
            service_type: SERVICE_WIN32_OWN_PROCESS,
            current_state: state,
            controls_accepted: controls,
            win32_exit_code: 0,
            service_specific_exit_code: 0,
            check_point: 0,
            wait_hint: 0,
        };
        unsafe {
            SetServiceStatus(STATUS_HANDLE, &mut status);
        }
    }
}
