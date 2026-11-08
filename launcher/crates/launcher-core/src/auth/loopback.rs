//! The one-shot loopback listener that receives the browser sign-in's redirect.
//!
//! It listens on `127.0.0.1` at a port the OS picks, and the redirect URI is
//! `http://localhost:<port>`. The app registration lists `http://localhost` as a
//! "Mobile and desktop" redirect URI, and the Microsoft identity platform ignores
//! the port of localhost redirect URIs when matching them; it does not support
//! `[::1]`. A redirect URI without a path comes back as `http://localhost:<port>/`.
//!
//! The listener accepts one redirect with a matching `state`, answers with a small
//! "you can close this tab" page and stops. A redirect with another `state` ends the
//! sign-in with [`AuthError::StateMismatch`]. It also stops when cancelled, when
//! dropped, or after the timeout.

use std::{
    net::{Ipv4Addr, SocketAddr, TcpListener},
    str::FromStr as _,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use futures::channel::oneshot;

use super::{AuthError, pkce::constant_time_eq};
use crate::Secret;

/// What the redirect carried.
type Outcome = Result<Secret, AuthError>;

/// A running listener. Await [`Loopback::wait`] for the authorization code.
pub struct Loopback {
    redirect_uri: String,
    port: u16,
    receiver: oneshot::Receiver<Outcome>,
    cancel: CancelOnDrop,
}

/// Stops the listener. Cloneable, so the UI can keep one while the sign-in task
/// owns the [`Loopback`].
#[derive(Clone)]
pub struct CancelHandle {
    cancelled: Arc<AtomicBool>,
    server: Arc<tiny_http::Server>,
}

impl CancelHandle {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        // Wakes the listener thread if it is waiting for a request.
        self.server.unblock();
    }
}

struct CancelOnDrop(CancelHandle);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

impl Loopback {
    /// Starts listening for a redirect carrying `state`, for at most `timeout`.
    pub fn start(state: String, timeout: Duration) -> Result<Self, AuthError> {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .map_err(|err| AuthError::Listener(err.to_string()))?;
        let port = listener
            .local_addr()
            .map_err(|err| AuthError::Listener(err.to_string()))?
            .port();
        let server = tiny_http::Server::from_listener(listener, None)
            .map_err(|err| AuthError::Listener(err.to_string()))?;
        let cancel = CancelHandle {
            cancelled: Arc::new(AtomicBool::new(false)),
            server: Arc::new(server),
        };

        let (sender, receiver) = oneshot::channel();
        let thread_cancel = cancel.clone();
        std::thread::Builder::new()
            .name("sign-in-loopback".into())
            .spawn(move || {
                let outcome = serve(&thread_cancel, &state, Instant::now() + timeout);
                // Release this thread's handle on the server first, so it closes as
                // soon as the caller has the outcome and drops the `Loopback`.
                drop(thread_cancel);
                let _ = sender.send(outcome);
            })
            .map_err(|err| AuthError::Listener(err.to_string()))?;

        Ok(Self {
            redirect_uri: format!("http://localhost:{port}"),
            port,
            receiver,
            cancel: CancelOnDrop(cancel),
        })
    }

    /// The `redirect_uri` for the authorization and token requests.
    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    /// The port on `127.0.0.1`.
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn cancel_handle(&self) -> CancelHandle {
        self.cancel.0.clone()
    }

    /// Waits for the redirect and returns the authorization code.
    pub async fn wait(self) -> Result<Secret, AuthError> {
        // `self.cancel` lives until this returns, so dropping the future stops the
        // listener.
        self.receiver.await.unwrap_or(Err(AuthError::Cancelled))
    }
}

fn serve(cancel: &CancelHandle, expected_state: &str, deadline: Instant) -> Outcome {
    loop {
        if cancel.cancelled.load(Ordering::SeqCst) {
            return Err(AuthError::Cancelled);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(AuthError::TimedOut);
        }
        let request = match cancel.server.recv_timeout(remaining) {
            Ok(Some(request)) => request,
            // Timed out or unblocked by a cancel; the checks above tell which.
            Ok(None) => continue,
            Err(err) => return Err(AuthError::Listener(err.to_string())),
        };
        match handle(&request, expected_state) {
            Handled::Ignore { status } => {
                let _ = request.respond(plain_response(status));
            }
            Handled::Done { outcome, page } => {
                let _ = request.respond(page_response(&outcome, page));
                return outcome;
            }
        }
    }
}

enum Handled {
    /// Not the redirect (for example a favicon request): answer and keep waiting.
    Ignore {
        status: u16,
    },
    Done {
        outcome: Outcome,
        page: Page,
    },
}

#[derive(Clone, Copy)]
enum Page {
    Success,
    Declined,
    Failed,
}

fn handle(request: &tiny_http::Request, expected_state: &str) -> Handled {
    if *request.method() != tiny_http::Method::Get {
        return Handled::Ignore { status: 405 };
    }
    let Ok(url) = url::Url::parse(&format!("http://localhost{}", request.url())) else {
        return Handled::Ignore { status: 400 };
    };
    if url.path() != "/" {
        return Handled::Ignore { status: 404 };
    }

    let (mut code, mut state, mut error) = (None, None, None);
    for (key, value) in url.query_pairs() {
        match &*key {
            "code" if !value.is_empty() => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" if !value.is_empty() => error = Some(value.into_owned()),
            _ => {}
        }
    }
    if code.is_none() && error.is_none() {
        return Handled::Ignore { status: 400 };
    }

    // Check `state` before anything else, so a request that did not come from our
    // authorization request cannot end the sign-in with a misleading error.
    if !state.is_some_and(|state| constant_time_eq(&state, expected_state)) {
        return Handled::Done {
            outcome: Err(AuthError::StateMismatch),
            page: Page::Failed,
        };
    }
    match (code, error) {
        (_, Some(error)) => {
            let declined = error == "access_denied";
            Handled::Done {
                outcome: Err(if declined {
                    AuthError::Declined
                } else {
                    AuthError::Microsoft { code: error }
                }),
                page: if declined {
                    Page::Declined
                } else {
                    Page::Failed
                },
            }
        }
        (Some(code), None) => Handled::Done {
            outcome: Ok(Secret::new(code)),
            page: Page::Success,
        },
        (None, None) => unreachable!("checked above"),
    }
}

fn header(line: &str) -> tiny_http::Header {
    tiny_http::Header::from_str(line).expect("a valid built-in header")
}

fn plain_response(status: u16) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    tiny_http::Response::from_string("")
        .with_status_code(status)
        .with_header(header("Connection: close"))
}

fn page_response(outcome: &Outcome, page: Page) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let (title, message) = match page {
        Page::Success => (
            "You're signed in",
            "You can close this tab and return to Lodestone Launcher.".to_string(),
        ),
        Page::Declined => (
            "Sign-in declined",
            "You can close this tab. Lodestone Launcher did not sign in.".to_string(),
        ),
        Page::Failed => (
            "Sign-in failed",
            match outcome {
                Err(err) => format!("{} You can close this tab.", err.message()),
                Ok(_) => String::new(),
            },
        ),
    };
    let status = if matches!(page, Page::Success) {
        200
    } else {
        400
    };
    tiny_http::Response::from_string(callback_page(title, &message))
        .with_status_code(status)
        .with_header(header("Content-Type: text/html; charset=utf-8"))
        .with_header(header("Cache-Control: no-store"))
        .with_header(header("Referrer-Policy: no-referrer"))
        .with_header(header(
            "Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'",
        ))
        .with_header(header("Connection: close"))
}

/// The page the browser shows after the redirect, in the dashboard's colours.
fn callback_page(title: &str, message: &str) -> String {
    let (title, message) = (escape(title), escape(message));
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} - Lodestone Launcher</title>
<style>
  body {{ margin: 0; min-height: 100vh; display: flex; align-items: center; justify-content: center;
    background: #212327; color: #E3E3E4; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }}
  main {{ max-width: 26rem; margin: 1rem; padding: 2.5rem; border-radius: 1rem; background: #26282C;
    border: 1px solid rgba(165, 165, 172, 0.3); text-align: center; }}
  h1 {{ margin: 0 0 0.75rem; font-size: 1.25rem; }}
  p {{ margin: 0; color: #A5A5AC; line-height: 1.5; }}
</style>
</head>
<body>
<main><h1>{title}</h1><p>{message}</p></main>
<script>history.replaceState(null, "", "/");</script>
</body>
</html>
"#
    )
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::io::{Read as _, Write as _};

    use futures::executor::block_on;

    use super::*;

    const LONG: Duration = Duration::from_secs(60);

    /// Sends a raw `GET` to the listener and returns the status code and body.
    fn get(port: u16, target: &str) -> (u16, String) {
        let mut stream = std::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        write!(
            stream,
            "GET {target} HTTP/1.1\r\nHost: localhost:{port}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let status = response[9..12].parse().unwrap();
        let body = response
            .split_once("\r\n\r\n")
            .map(|(_, body)| body.to_string())
            .unwrap_or_default();
        (status, body)
    }

    #[test]
    fn returns_the_code_for_the_matching_state() {
        let loopback = Loopback::start("expected".into(), LONG).unwrap();
        assert_eq!(
            loopback.redirect_uri(),
            format!("http://localhost:{}", loopback.port())
        );
        let port = loopback.port();
        // Unrelated requests are answered and ignored.
        assert_eq!(get(port, "/favicon.ico").0, 404);
        assert_eq!(get(port, "/").0, 400);

        let (status, body) = get(port, "/?code=the-code&state=expected");
        assert_eq!(status, 200);
        assert!(body.contains("You can close this tab"), "{body}");
        assert!(!body.contains("the-code"));
        assert_eq!(block_on(loopback.wait()).unwrap().expose(), "the-code");
    }

    #[test]
    fn rejects_a_state_mismatch() {
        let loopback = Loopback::start("expected".into(), LONG).unwrap();
        let (status, body) = get(loopback.port(), "/?code=stolen&state=other");
        assert_eq!(status, 400);
        assert!(body.contains("Sign-in failed"), "{body}");
        assert_eq!(block_on(loopback.wait()), Err(AuthError::StateMismatch));
    }

    #[test]
    fn rejects_a_missing_state() {
        let loopback = Loopback::start("expected".into(), LONG).unwrap();
        assert_eq!(get(loopback.port(), "/?code=stolen").0, 400);
        assert_eq!(block_on(loopback.wait()), Err(AuthError::StateMismatch));
    }

    #[test]
    fn reports_a_declined_consent() {
        let loopback = Loopback::start("s".into(), LONG).unwrap();
        let (status, body) = get(
            loopback.port(),
            "/?error=access_denied&error_description=The+user+declined&state=s",
        );
        assert_eq!(status, 400);
        assert!(body.contains("Sign-in declined"), "{body}");
        assert_eq!(block_on(loopback.wait()), Err(AuthError::Declined));

        let loopback = Loopback::start("s".into(), LONG).unwrap();
        get(loopback.port(), "/?error=server_error&state=s");
        assert_eq!(
            block_on(loopback.wait()),
            Err(AuthError::Microsoft {
                code: "server_error".into()
            })
        );
    }

    #[test]
    fn times_out() {
        let loopback = Loopback::start("s".into(), Duration::from_millis(20)).unwrap();
        assert_eq!(block_on(loopback.wait()), Err(AuthError::TimedOut));
    }

    #[test]
    fn can_be_cancelled() {
        let loopback = Loopback::start("s".into(), LONG).unwrap();
        let handle = loopback.cancel_handle();
        handle.cancel();
        assert_eq!(block_on(loopback.wait()), Err(AuthError::Cancelled));
    }

    #[test]
    fn dropping_it_stops_the_listener() {
        let loopback = Loopback::start("s".into(), LONG).unwrap();
        let handle = loopback.cancel_handle();
        drop(loopback);
        assert!(handle.cancelled.load(Ordering::SeqCst));
    }

    #[test]
    fn escapes_the_page() {
        let page = callback_page("<t>", "a & \"b\"");
        assert!(page.contains("&lt;t&gt;"));
        assert!(page.contains("a &amp; &quot;b&quot;"));
    }
}
