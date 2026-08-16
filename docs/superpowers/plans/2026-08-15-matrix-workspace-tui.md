# matrix-workspace-tui Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a desktop TUI client for the Matrix Agent Workspace control plane in Rust (ratatui), delivered via npx as a thin npm launcher around prebuilt platform binaries.

**Architecture:** A Cargo workspace with three crates: `api-client` (reqwest + tokio; typed HTTP for every control-plane route the mobile app uses, plus a resumable SSE event stream), `state` (session store on disk with mode-0600 permissions, the screen state machine, and pure per-screen state), and `tui` (the ratatui binary: event loop, screen router, one screen per navigation stop). An `npm/matrix-workspace-tui` package downloads a checksum-verified platform binary in `postinstall` and execs it via its `bin` entry; GitHub Actions CI tests/builds on linux-x64, linux-aarch64, darwin-x64, darwin-aarch64 and publishes binaries + npm package.

**Tech Stack:** Rust (workspace: `api-client`, `state`, `tui`), tokio, reqwest (rustls, json, stream), serde/serde_json, thiserror, uuid, sha2, ratatui, crossterm, dirs, httpmock (dev, mock HTTP server for tests), tempfile (dev); npm launcher package (Node ≥ 18, `node:test` for tests); GitHub Actions CI.

---

# How to work through this plan

- Work **one task at a time**, top to bottom. Each task is 2–5 minutes and ends with a commit on branch `fm/matrix-tui-plan-001`.
- **TDD shape** for every code task: (1) write the failing test, (2) run it and confirm it FAILS, (3) write the minimal implementation, (4) run it and confirm it PASSES, (5) commit.
- The backend API contract is authoritative and read-only. It lives in the repo `abderrazzakchabab/matrix-agent-workspace` at `apps/mobile/src/api/control-plane.ts`, `apps/mobile/src/api/run-events.ts`, and `packages/contracts/src/{events,run,github,errors}.ts` (branch `main`, commit `063e2e1`). The Rust types in this plan mirror those shapes field-for-field (camelCase JSON, same enums, same status codes). **Do not invent fields or endpoints.**
- One deliberate contract note: the backend has **no** `GET /api/workspaces` list endpoint (verified: `apps/control-plane/src/app/api/workspaces/route.ts` only defines `POST`). The Workspaces screen therefore lists workspaces this client has created, persisted locally in the session file; creation still goes through `POST /api/workspaces`.
- The GitHub read routes require an `installationId` query param. The mobile app takes it from `EXPO_PUBLIC_GITHUB_INSTALLATION_ID`; the TUI takes it from the env var `MATRIX_WORKSPACE_TUI_GITHUB_INSTALLATION_ID`. When unset, the GitHubWorkspace screen shows an "unlinked" state and mutation controls stay hidden (same as the mobile).
- The SSH-free way to check a failing test's expected output: when the test references a function that does not exist yet, `cargo test` fails to compile with `error[E0425]: cannot find function ... in this scope`. That compile error IS the expected failure. When the function exists but the assertion is wrong, the failure is `test result: FAILED` with an assertion message.
- Rust toolchain: `rust-toolchain.toml` pins 1.85.0. If a transitive dependency demands a newer compiler, bump the `channel` value (do not delete the file). Install via rustup: `rustup toolchain install 1.85.0`.
- Every commit message follows the plan's `git commit -m "..."` line exactly.
- Do not run `cargo fmt` reformatting on the whole tree between tasks unless a task says so; the code blocks are already formatted.

## Mock server decision

All api-client and tui tests use **httpmock 0.7** (`httpmock = "0.7"`), a tokio-native mock HTTP server. Its API (verified against 0.7.0):

```rust
use httpmock::prelude::*;
let server = MockServer::start_async().await;
let mock = server.mock_async(|when, then| {
    when.method(GET).path("/api/rooms").header("cookie", "sid=abc");
    then.status(200).json_body(json!({ "rooms": [] }));
}).await;
let url = server.url("/api/rooms");   // absolute URL
let base = server.base_url();          // http://127.0.0.1:<port>
mock.assert_async().await;             // assert the mock was hit
assert!(mock.hits_async().await >= 1); // count hits
```

Query params are matched with `when.query_param("after", "0")`. SSE bodies use `then.status(200).header("content-type", "text/event-stream").body(sse_text)`.

## File structure (everything this plan creates)

```
matrix-workspace-tui/
├── Cargo.toml                          # workspace root (members, shared deps, release profile)
├── rust-toolchain.toml                 # pinned channel 1.85.0
├── .gitignore                          # extended (existing file)
├── crates/
│   ├── api-client/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # module wiring + re-exports
│   │       ├── error.rs                # ControlPlaneError + ApiErrorBody
│   │       ├── contract.rs             # typed wire structs mirroring control-plane.ts / contracts/*
│   │       ├── http.rs                 # ControlPlaneApi core: cookie, authenticated_request, login_request, urlencode
│   │       ├── sse.rs                  # SseFrame, parse_sse_frame, RunEvent::from_sse_frame, RunEventBuffer, EventStream
│   │       └── api/
│   │           ├── mod.rs
│   │           ├── auth.rs             # create_matrix_session
│   │           ├── workspaces.rs       # create_workspace
│   │           ├── rooms.rs            # get_rooms, bind_room
│   │           ├── runs.rs             # launch_run, cancel_run, get_run_matrix_deliveries, create_run_approval
│   │           ├── github.rs           # list_repositories/issues/pulls, request_write_grant, enqueue_mutation
│   │           └── audit.rs            # list_audit_records
│   ├── state/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                  # module wiring + re-exports
│   │       ├── session_store.rs        # SessionData, SessionStore (0600), StateError
│   │       └── screens.rs              # ScreenId, Screen, per-screen state structs, mutation helpers
│   └── tui/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs                 # env config, terminal setup, App::run
│           ├── app.rs                  # App, Command, AppEvent, run loop, command executor, router
│           ├── input.rs                # InputBuffer for text fields
│           └── screens/
│               ├── mod.rs              # re-exports
│               ├── login.rs
│               ├── workspaces.rs
│               ├── rooms.rs
│               ├── run_composer.rs
│               ├── run.rs
│               └── github.rs
├── npm/
│   └── matrix-workspace-tui/
│       ├── package.json
│       ├── index.js                    # bin: spawns the downloaded binary
│       ├── scripts/download.js         # postinstall: platform binary + sha256 verify
│       └── test/
│           ├── download.test.js        # node:test with local static server
│           └── launcher.test.js
└── .github/workflows/
    ├── ci.yml                          # test+build per platform
    └── release.yml                     # four binaries -> GitHub release -> npm publish
```

---

# Group 1: Workspace scaffolding

### Task 1.1: Create the workspace root files

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Modify: `.gitignore` (append two lines)

- [x] **Step 1: Write `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = ["crates/api-client", "crates/state", "crates/tui"]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.85"

[workspace.dependencies]
tokio = { version = "1.40", features = ["rt-multi-thread", "macros", "time"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "stream"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
futures-util = "0.3"
uuid = { version = "1", features = ["v4"] }
dirs = "5"
sha2 = "0.10"
ratatui = { version = "0.29", features = ["crossterm"] }
crossterm = "0.28"
httpmock = "0.7"
tempfile = "3"

[profile.release]
lto = true
strip = true
```

- [x] **Step 2: Write `rust-toolchain.toml`**

```toml
[toolchain]
channel = "1.85.0"
components = ["rustfmt", "clippy"]
```

- [x] **Step 3: Append to `.gitignore`**

```gitignore

# Rust analysis
rust-toolchain.toml.old
```

- [x] **Step 4: Verify the workspace parses**

Run: `cargo metadata --no-deps --format-version 1 >/dev/null`
Expected: `error: failed to load manifest ... no such file or directory` for `crates/api-client/Cargo.toml` — this is fine, the members do not exist yet. Do **not** treat this as a blocker; proceed to Task 1.2 which creates the members.

- [x] **Step 5: Commit**

```bash
git add Cargo.toml rust-toolchain.toml .gitignore
git commit -m "chore: add cargo workspace root and pinned toolchain"
```

### Task 1.2: Create the `api-client` crate skeleton with base dependencies

**Files:**
- Create: `crates/api-client/Cargo.toml`
- Create: `crates/api-client/src/lib.rs`

- [x] **Step 1: Write `crates/api-client/Cargo.toml`**

```toml
[package]
name = "api-client"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
tokio.workspace = true
reqwest.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
futures-util.workspace = true
uuid.workspace = true

[dev-dependencies]
httpmock.workspace = true
```

- [x] **Step 2: Write `crates/api-client/src/lib.rs` (placeholder, expanded in later groups)**

```rust
//! Typed HTTP + SSE client for the Matrix Agent Workspace control plane.
```

- [x] **Step 3: Build the crate**

Run: `cargo build -p api-client`
Expected: `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in ...` (first run compiles the dependency tree: tokio, reqwest, serde, httpmock — this takes a few minutes).

- [x] **Step 4: Commit**

```bash
git add crates/api-client/Cargo.toml crates/api-client/src/lib.rs
git commit -m "chore: scaffold api-client crate with base dependencies"
```

### Task 1.3: Create the `state` and `tui` crate skeletons

**Files:**
- Create: `crates/state/Cargo.toml`
- Create: `crates/state/src/lib.rs`
- Create: `crates/tui/Cargo.toml`
- Create: `crates/tui/src/main.rs`

- [x] **Step 1: Write `crates/state/Cargo.toml`**

```toml
[package]
name = "state"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
api-client = { path = "../api-client" }
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
dirs.workspace = true
sha2.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

- [x] **Step 2: Write `crates/state/src/lib.rs` (placeholder, expanded in later groups)**

```rust
//! Session persistence and the screen state machine for matrix-workspace-tui.
```

- [x] **Step 3: Write `crates/tui/Cargo.toml`**

```toml
[package]
name = "tui"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[[bin]]
name = "matrix-workspace-tui"
path = "src/main.rs"

[dependencies]
api-client = { path = "../api-client" }
state = { path = "../state" }
tokio.workspace = true
serde_json.workspace = true
thiserror.workspace = true
uuid.workspace = true
ratatui.workspace = true
crossterm.workspace = true

[dev-dependencies]
httpmock.workspace = true
tempfile.workspace = true
```

- [x] **Step 4: Write `crates/tui/src/main.rs` (placeholder, expanded in Group 7)**

```rust
fn main() {
    eprintln!("matrix-workspace-tui: build placeholder");
    std::process::exit(1);
}
```

- [x] **Step 5: Build the whole workspace**

Run: `cargo build --workspace`
Expected: `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in ...`

- [x] **Step 6: Commit**

```bash
git add crates/state crates/tui
git commit -m "chore: scaffold state and tui crates"
```

---

# Group 2: api-client — HTTP core

### Task 2.1: `ControlPlaneError` + API error body types

**Files:**
- Create: `crates/api-client/src/error.rs`
- Modify: `crates/api-client/src/lib.rs`

- [ ] **Step 1: Write the failing test (in `crates/api-client/src/error.rs`, appended below the types — no, tests live in the same file as `#[cfg(test)]`)**

Create `crates/api-client/src/error.rs` with this test-only-free content first, then the test is added together with the impl in the TDD shape. To keep the TDD loop honest, write the test first inside a `#[cfg(test)] mod tests` at the bottom of the file, referencing the not-yet-written types:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn api_error(status: u16, code: &str, message: &str) -> ControlPlaneError {
        ControlPlaneError::Api {
            status,
            code: Some(code.to_string()),
            message: message.to_string(),
        }
    }

    #[test]
    fn api_error_exposes_status_code_and_message() {
        let error = api_error(422, "VALIDATION_ERROR", "Invalid workspace");
        assert_eq!(error.status(), 422);
        assert_eq!(error.code(), Some("VALIDATION_ERROR"));
        assert_eq!(error.to_string(), "control plane returned 422: Invalid workspace");
    }

    #[test]
    fn api_error_without_code_formats_cleanly() {
        let error = ControlPlaneError::Api {
            status: 500,
            code: None,
            message: "boom".to_string(),
        };
        assert_eq!(error.to_string(), "control plane returned 500: boom");
    }

    #[test]
    fn session_expired_is_a_distinct_signal() {
        let error = ControlPlaneError::SessionExpired;
        assert!(error.is_session_expired());
        assert!(!api_error(401, "SESSION_REQUIRED", "Sign in again").is_session_expired());
        assert_eq!(error.to_string(), "session expired; sign in again");
    }

    #[test]
    fn error_body_parses_optional_fields() {
        let json = r#"{"error":{"code":"VALIDATION_ERROR","message":"Invalid workspace","requestId":"req_1"}}"#;
        let body: ApiErrorBody = serde_json::from_str(json).unwrap();
        assert_eq!(body.error.code.as_deref(), Some("VALIDATION_ERROR"));
        assert_eq!(body.error.message.as_deref(), Some("Invalid workspace"));
        let body: ApiErrorBody = serde_json::from_str(r#"{"error":{}}"#).unwrap();
        assert_eq!(body.error.code, None);
    }
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client error_body_parses_optional_fields`
Expected: `error[E0433]: failed to resolve: use of undeclared type \`ApiErrorBody\`` (and similar E0433/E0432 for `ControlPlaneError`).

- [ ] **Step 3: Write the implementation — full `crates/api-client/src/error.rs`**

```rust
use thiserror::Error;

/// Structured error body returned by control-plane endpoints
/// (mirrors `ApiError` in packages/contracts/src/errors.ts; fields kept
/// optional because some endpoints return partial bodies).
#[derive(Debug, serde::Deserialize)]
pub struct ApiErrorBody {
    pub error: ApiErrorDetail,
}

#[derive(Debug, serde::Deserialize)]
pub struct ApiErrorDetail {
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Error)]
pub enum ControlPlaneError {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("no control-plane session stored; sign in first")]
    NoSession,

    #[error("session expired; sign in again")]
    SessionExpired,

    #[error("the control plane did not return a session reference")]
    SessionReferenceMissing,

    #[error("control plane returned {status}: {message}")]
    Api {
        status: u16,
        code: Option<String>,
        message: String,
    },

    #[error("unexpected response from control plane: {0}")]
    InvalidResponse(String),

    #[error("invalid control plane base url")]
    InvalidBaseUrl,
}

impl ControlPlaneError {
    pub fn status(&self) -> Option<u16> {
        match self {
            ControlPlaneError::Api { status, .. } => Some(*status),
            _ => None,
        }
    }

    pub fn code(&self) -> Option<&str> {
        match self {
            ControlPlaneError::Api { code, .. } => code.as_deref(),
            _ => None,
        }
    }

    pub fn is_session_expired(&self) -> bool {
        matches!(self, ControlPlaneError::SessionExpired)
    }
}
```

- [ ] **Step 4: Wire the module into `crates/api-client/src/lib.rs`**

```rust
//! Typed HTTP + SSE client for the Matrix Agent Workspace control plane.

pub mod error;

pub use error::{ApiErrorBody, ApiErrorDetail, ControlPlaneError};
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test -p api-client`
Expected: `test result: ok. 4 passed; 0 failed; ...`

- [ ] **Step 6: Commit**

```bash
git add crates/api-client/src/error.rs crates/api-client/src/lib.rs
git commit -m "feat(api-client): typed control-plane error with session-expired signal"
```

### Task 2.2: `ControlPlaneApi::new`, cookie accessors, and `urlencode`

**Files:**
- Create: `crates/api-client/src/http.rs`
- Modify: `crates/api-client/src/lib.rs`

- [ ] **Step 1: Write the failing test (in `crates/api-client/src/http.rs`, `#[cfg(test)] mod tests`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_normalizes_base_url_and_rejects_empty() {
        let client = ControlPlaneApi::new("  http://localhost:3000/// ").unwrap();
        assert_eq!(client.base_url(), "http://localhost:3000");
        assert!(ControlPlaneApi::new("   ").is_err());
    }

    #[test]
    fn cookie_accessors_default_to_none() {
        let mut client = ControlPlaneApi::new("http://localhost:3000").unwrap();
        assert_eq!(client.cookie(), None);
        client.set_cookie(Some("sid=abc".to_string()));
        assert_eq!(client.cookie(), Some("sid=abc"));
        client.set_cookie(None);
        assert_eq!(client.cookie(), None);
    }

    #[test]
    fn urlencode_matches_js_encodeuricomponent_semantics() {
        assert_eq!(urlencode("!room:example.org"), "%21room%3Aexample.org");
        assert_eq!(urlencode("a b&c=d"), "a%20b%26c%3Dd");
        assert_eq!(urlencode("plain-id_1.2~3"), "plain-id_1.2~3");
    }
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client urlencode_matches_js_encodeuricomponent_semantics`
Expected: `error[E0425]: cannot find function \`urlencode\` in this scope` / `cannot find type \`ControlPlaneApi\``.

- [ ] **Step 3: Write the implementation — full `crates/api-client/src/http.rs`**

```rust
use crate::error::{ApiErrorBody, ControlPlaneError};
use reqwest::StatusCode;

/// Percent-encode a path segment the same way JavaScript's
/// `encodeURIComponent` does (used for room ids, run ids, owner/repo names).
pub(crate) fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// The control-plane client: a base URL plus the session cookie. All
/// authenticated methods mirror `ControlPlaneApi` in
/// apps/mobile/src/api/control-plane.ts.
pub struct ControlPlaneApi {
    http: reqwest::Client,
    base_url: String,
    cookie: Option<String>,
}

impl ControlPlaneApi {
    pub fn new(base_url: impl Into<String>) -> Result<Self, ControlPlaneError> {
        let base_url = base_url.into().trim().trim_end_matches('/').to_string();
        if base_url.is_empty() {
            return Err(ControlPlaneError::InvalidBaseUrl);
        }
        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            cookie: None,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn set_cookie(&mut self, cookie: Option<String>) {
        self.cookie = cookie;
    }

    pub fn cookie(&self) -> Option<&str> {
        self.cookie.as_deref()
    }

    pub(crate) fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// Send an authenticated request and return the response status + parsed
    /// JSON body. A 401 anywhere means the session cookie is no longer valid
    /// and maps to the `SessionExpired` signal.
    pub(crate) async fn authenticated_response(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<(StatusCode, serde_json::Value), ControlPlaneError> {
        let cookie = self.cookie.as_deref().ok_or(ControlPlaneError::NoSession)?;
        let mut request = self
            .http
            .request(method, format!("{}{}", self.base_url, path))
            .header(reqwest::header::COOKIE, cookie)
            .header(reqwest::header::ACCEPT, "application/json");
        if let Some(body) = body {
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .json(body);
        }
        let response = request.send().await.map_err(ControlPlaneError::Http)?;
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(ControlPlaneError::SessionExpired);
        }
        let status = response.status();
        let bytes = response.bytes().await.map_err(ControlPlaneError::Http)?;
        if status.is_success() {
            let value: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|e| ControlPlaneError::InvalidResponse(e.to_string()))?;
            Ok((status, value))
        } else {
            let api_error: Option<ApiErrorBody> = serde_json::from_slice(&bytes).ok();
            Err(ControlPlaneError::Api {
                status: status.as_u16(),
                code: api_error.and_then(|body| body.error.code),
                message: api_error
                    .map(|body| body.error.message.unwrap_or_default())
                    .unwrap_or_else(|| format!("Control plane request failed ({status})")),
            })
        }
    }

    /// Send an authenticated request and deserialize the JSON body into `T`.
    pub(crate) async fn authenticated_request<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<T, ControlPlaneError> {
        let (_, value) = self.authenticated_response(method, path, body).await?;
        serde_json::from_value(value).map_err(|e| ControlPlaneError::InvalidResponse(e.to_string()))
    }

    /// POST with a JSON body but no cookie (only used by
    /// `create_matrix_session`). Returns the parsed body plus the `Set-Cookie`
    /// value if the server issued one.
    pub(crate) async fn login_request<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<(T, Option<String>), ControlPlaneError> {
        let response = self
            .http
            .post(format!("{}{}", self.base_url, path))
            .header(reqwest::header::ACCEPT, "application/json")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .json(body)
            .send()
            .await
            .map_err(ControlPlaneError::Http)?;
        let status = response.status();
        let set_cookie = response
            .headers()
            .get(reqwest::header::SET_COOKIE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.split(';').next().unwrap_or("").trim().to_string())
            .filter(|cookie| !cookie.is_empty());
        let bytes = response.bytes().await.map_err(ControlPlaneError::Http)?;
        if status.is_success() {
            let value = serde_json::from_slice(&bytes)
                .map_err(|e| ControlPlaneError::InvalidResponse(e.to_string()))?;
            Ok((value, set_cookie))
        } else {
            let api_error: Option<ApiErrorBody> = serde_json::from_slice(&bytes).ok();
            Err(ControlPlaneError::Api {
                status: status.as_u16(),
                code: api_error.and_then(|body| body.error.code),
                message: api_error
                    .map(|body| body.error.message.unwrap_or_default())
                    .unwrap_or_else(|| format!("Control plane request failed ({status})")),
            })
        }
    }
}
```

- [ ] **Step 4: Wire the module into `crates/api-client/src/lib.rs`**

```rust
//! Typed HTTP + SSE client for the Matrix Agent Workspace control plane.

pub mod error;
pub mod http;

pub use error::{ApiErrorBody, ApiErrorDetail, ControlPlaneError};
pub use http::ControlPlaneApi;
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test -p api-client`
Expected: `test result: ok. 7 passed; 0 failed; ...`

- [ ] **Step 6: Commit**

```bash
git add crates/api-client/src/http.rs crates/api-client/src/lib.rs
git commit -m "feat(api-client): http core with cookie handling and urlencode"
```

### Task 2.3: `authenticated_request` — success, 401, and error-body mapping (mock server)

**Files:**
- Modify: `crates/api-client/src/http.rs` (append tests)

- [ ] **Step 1: Write the failing tests (append to the `tests` module in `crates/api-client/src/http.rs`)**

```rust
use httpmock::prelude::*;
use serde_json::json;

#[tokio::test]
async fn authenticated_request_sends_cookie_and_parses_body() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/api/rooms")
                .header("cookie", "sid=abc");
            then.status(200).json_body(json!({ "rooms": [] }));
        })
        .await;
    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("sid=abc".to_string()));
    let value: serde_json::Value = client
        .authenticated_request(reqwest::Method::GET, "/api/rooms", None)
        .await
        .unwrap();
    assert_eq!(value["rooms"], json!([]));
    mock.assert_async().await;
}

#[tokio::test]
async fn authenticated_request_maps_401_to_session_expired() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/api/rooms");
            then.status(401).json_body(json!({
                "error": { "code": "SESSION_REQUIRED", "message": "Sign in again", "requestId": "req_1" }
            }));
        })
        .await;
    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("sid=abc".to_string()));
    let error = client
        .authenticated_request::<serde_json::Value>(reqwest::Method::GET, "/api/rooms", None)
        .await
        .unwrap_err();
    assert!(error.is_session_expired());
}

#[tokio::test]
async fn authenticated_request_maps_non_ok_status_to_typed_error() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/api/workspaces");
            then.status(422).json_body(json!({
                "error": { "code": "VALIDATION_ERROR", "message": "Invalid workspace", "requestId": "req_1" }
            }));
        })
        .await;
    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("sid=abc".to_string()));
    let error = client
        .authenticated_request::<serde_json::Value>(reqwest::Method::GET, "/api/workspaces", None)
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(422));
    assert_eq!(error.code(), Some("VALIDATION_ERROR"));
    assert_eq!(
        error.to_string(),
        "control plane returned 422: Invalid workspace"
    );
}

#[tokio::test]
async fn authenticated_request_without_session_is_no_session() {
    let client = ControlPlaneApi::new("http://localhost:9").unwrap();
    let error = client
        .authenticated_request::<serde_json::Value>(reqwest::Method::GET, "/api/rooms", None)
        .await
        .unwrap_err();
    assert!(matches!(error, ControlPlaneError::NoSession));
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p api-client authenticated_request`
Expected: `test result: FAILED. 0 passed; 4 failed` — the methods exist (Task 2.2) but the tests fail because... wait, they should already pass. To force the TDD loop, the real failure to confirm first: `cargo test -p api-client authenticated_request_sends_cookie_and_parses_body` when the mock returns `{"rooms":[]}` but the assertion expects `[]` — no. Instead, run the suite once now: it compiles (methods exist) and **passes**. To keep the red-green loop meaningful, first add the tests, run, and confirm they compile and pass (they test Task 2.2 behavior end-to-end); then in Step 4 make a deliberate behavior improvement (POST JSON bodies) with its own red test.

If you want to see an actual red run first: temporarily change the mock path in the first test to `/api/rooms2`, run, see `test result: FAILED` with `Expected GET http://.../api/rooms2 to be called, but it wasn't` (mock not hit), then revert the path. This confirms the mock harness is wired.

- [ ] **Step 3: Add the POST-with-body test (this is the new behavior being TDD'd)**

Append:

```rust
#[tokio::test]
async fn authenticated_request_posts_json_body() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/api/workspaces")
                .header("content-type", "application/json")
                .body_contains(r#""name":"my workspace""#)
                .body_contains(r#""readOnly":true"#);
            then.status(201).json_body(json!({
                "requestId": "req_1",
                "workspaceId": "ws_1",
                "name": "my workspace",
                "ownerId": "@u:example.org",
                "status": "active",
                "createdAt": "2026-08-15T00:00:00.000Z"
            }));
        })
        .await;
    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("sid=abc".to_string()));
    let body = json!({ "name": "my workspace", "policy": { "readOnly": true } });
    let value: serde_json::Value = client
        .authenticated_request(reqwest::Method::POST, "/api/workspaces", Some(&body))
        .await
        .unwrap();
    assert_eq!(value["workspaceId"], "ws_1");
    mock.assert_async().await;
}
```

Run: `cargo test -p api-client authenticated_request_posts_json_body`
Expected: FAIL on first run — the request body is sent as `application/json` and contains the fields, so it passes. If instead you want a guaranteed red: change `.body_contains(r#""name":"my workspace""#)` to `.body_contains(r#""name":"different""#)` → `test result: FAILED` with the mock-body mismatch, then revert. (This is a mock-harness sanity check, not a product bug.)

- [ ] **Step 4: Run the full api-client suite**

Run: `cargo test -p api-client`
Expected: `test result: ok. 12 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/http.rs
git commit -m "test(api-client): authenticated request against mock server"
```

---

# Group 3: api-client — resources

Every task in this group appends the wire types it needs to `crates/api-client/src/contract.rs` (fields mirror `apps/mobile/src/api/control-plane.ts` and `packages/contracts/src/*.ts` exactly — camelCase JSON on the wire) and adds one method + its tests to the matching `crates/api-client/src/api/<resource>.rs`. The `ControlPlaneApi` struct from Group 2 gets one `impl` block per resource module.

### Task 3.1: `create_matrix_session` (auth)

**Files:**
- Create: `crates/api-client/src/contract.rs` (auth types)
- Create: `crates/api-client/src/api/mod.rs`
- Create: `crates/api-client/src/api/auth.rs`
- Modify: `crates/api-client/src/lib.rs`

- [ ] **Step 1: Write the failing test — `crates/api-client/src/api/auth.rs`**

```rust
use crate::contract::MatrixSessionResponse;
use crate::http::ControlPlaneApi;
use httpmock::prelude::*;
use serde_json::json;

#[tokio::test]
async fn create_matrix_session_stores_set_cookie() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/api/auth/matrix/session")
                .body_contains(r#""homeserverUrl":"https://matrix.example.org""#)
                .body_contains(r#""accessToken":"tok_123""#);
            then.status(200)
                .header("set-cookie", "cp_session=abc123; Path=/; HttpOnly")
                .json_body(json!({
                    "user": { "id": "@u:matrix.example.org", "homeserverUrl": "https://matrix.example.org" },
                    "sessionExpiresAt": "2026-08-15T01:00:00.000Z"
                }));
        })
        .await;

    let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
    let session: MatrixSessionResponse = client
        .create_matrix_session("https://matrix.example.org", "tok_123")
        .await
        .unwrap();

    assert_eq!(session.user.id, "@u:matrix.example.org");
    assert_eq!(session.session_expires_at, "2026-08-15T01:00:00.000Z");
    assert_eq!(client.cookie(), Some("cp_session=abc123"));
    mock.assert_async().await;
}

#[tokio::test]
async fn create_matrix_session_without_cookie_is_reference_missing() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(POST).path("/api/auth/matrix/session");
            then.status(200).json_body(json!({
                "user": { "id": "@u:matrix.example.org", "homeserverUrl": "https://matrix.example.org" },
                "sessionExpiresAt": "2026-08-15T01:00:00.000Z"
            }));
        })
        .await;

    let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
    let error = client
        .create_matrix_session("https://matrix.example.org", "tok_123")
        .await
        .unwrap_err();
    assert!(matches!(error, crate::ControlPlaneError::SessionReferenceMissing));
    assert_eq!(client.cookie(), None);
}

#[tokio::test]
async fn create_matrix_session_invalid_credentials_surfaces_api_error() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(POST).path("/api/auth/matrix/session");
            then.status(401).json_body(json!({
                "error": { "code": "MATRIX_AUTH_FAILED", "message": "Invalid homeserver or token", "requestId": "req_1" }
            }));
        })
        .await;

    let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
    let error = client
        .create_matrix_session("https://matrix.example.org", "bad")
        .await
        .unwrap_err();
    assert_eq!(error.status(), Some(401));
    assert_eq!(error.code(), Some("MATRIX_AUTH_FAILED"));
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client create_matrix_session`
Expected: compile error — `cannot find method \`create_matrix_session\` for struct \`ControlPlaneApi\``.

- [ ] **Step 3: Write the implementation**

`crates/api-client/src/contract.rs` (new file — auth types only for now):

```rust
use serde::{Deserialize, Serialize};

/// POST /api/auth/matrix/session response (mirrors MatrixSessionResponse).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSessionResponse {
    pub user: MatrixUser,
    pub session_expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixUser {
    pub id: String,
    pub homeserver_url: String,
}

/// POST /api/auth/matrix/session request body.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixSessionRequest {
    pub homeserver_url: String,
    pub access_token: String,
}
```

`crates/api-client/src/api/mod.rs`:

```rust
pub mod auth;
pub mod workspaces;
pub mod rooms;
pub mod runs;
pub mod github;
pub mod audit;

pub use auth::*;
pub use workspaces::*;
pub use rooms::*;
pub use runs::*;
pub use github::*;
pub use audit::*;
```

`crates/api-client/src/api/auth.rs` (implementation + tests):

```rust
use crate::contract::{MatrixSessionRequest, MatrixSessionResponse};
use crate::error::ControlPlaneError;
use crate::http::ControlPlaneApi;
use serde_json::json;

impl ControlPlaneApi {
    /// POST /api/auth/matrix/session — exchange a homeserver URL + Matrix
    /// access token for a control-plane session cookie. The cookie is stored
    /// on this client (the caller persists it via the SessionStore).
    pub async fn create_matrix_session(
        &mut self,
        homeserver_url: &str,
        access_token: &str,
    ) -> Result<MatrixSessionResponse, ControlPlaneError> {
        let request = MatrixSessionRequest {
            homeserver_url: homeserver_url.to_string(),
            access_token: access_token.to_string(),
        };
        let body = json!(request);
        let (response, cookie) = self.login_request("/api/auth/matrix/session", &body).await?;
        match cookie {
            Some(cookie) => {
                self.set_cookie(Some(cookie));
                Ok(response)
            }
            None => Err(ControlPlaneError::SessionReferenceMissing),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn create_matrix_session_stores_set_cookie() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/api/auth/matrix/session")
                    .body_contains(r#""homeserverUrl":"https://matrix.example.org""#)
                    .body_contains(r#""accessToken":"tok_123""#);
                then.status(200)
                    .header("set-cookie", "cp_session=abc123; Path=/; HttpOnly")
                    .json_body(json!({
                        "user": { "id": "@u:matrix.example.org", "homeserverUrl": "https://matrix.example.org" },
                        "sessionExpiresAt": "2026-08-15T01:00:00.000Z"
                    }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        let session: MatrixSessionResponse = client
            .create_matrix_session("https://matrix.example.org", "tok_123")
            .await
            .unwrap();

        assert_eq!(session.user.id, "@u:matrix.example.org");
        assert_eq!(session.session_expires_at, "2026-08-15T01:00:00.000Z");
        assert_eq!(client.cookie(), Some("cp_session=abc123"));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn create_matrix_session_without_cookie_is_reference_missing() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/api/auth/matrix/session");
                then.status(200).json_body(json!({
                    "user": { "id": "@u:matrix.example.org", "homeserverUrl": "https://matrix.example.org" },
                    "sessionExpiresAt": "2026-08-15T01:00:00.000Z"
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        let error = client
            .create_matrix_session("https://matrix.example.org", "tok_123")
            .await
            .unwrap_err();
        assert!(matches!(error, crate::ControlPlaneError::SessionReferenceMissing));
        assert_eq!(client.cookie(), None);
    }

    #[tokio::test]
    async fn create_matrix_session_invalid_credentials_surfaces_api_error() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(POST).path("/api/auth/matrix/session");
                then.status(401).json_body(json!({
                    "error": { "code": "MATRIX_AUTH_FAILED", "message": "Invalid homeserver or token", "requestId": "req_1" }
                }));
            })
            .await;

        let mut client = ControlPlaneApi::new(server.base_url()).unwrap();
        let error = client
            .create_matrix_session("https://matrix.example.org", "bad")
            .await
            .unwrap_err();
        assert_eq!(error.status(), Some(401));
        assert_eq!(error.code(), Some("MATRIX_AUTH_FAILED"));
    }
}
```

- [ ] **Step 4: Wire the modules into `crates/api-client/src/lib.rs`**

```rust
//! Typed HTTP + SSE client for the Matrix Agent Workspace control plane.

pub mod api;
pub mod contract;
pub mod error;
pub mod http;

pub use api::*;
pub use contract::*;
pub use error::{ApiErrorBody, ApiErrorDetail, ControlPlaneError};
pub use http::ControlPlaneApi;
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test -p api-client`
Expected: `test result: ok. 15 passed; 0 failed; ...`

- [ ] **Step 6: Commit**

```bash
git add crates/api-client/src/contract.rs crates/api-client/src/api crates/api-client/src/lib.rs
git commit -m "feat(api-client): create_matrix_session with set-cookie capture"
```

### Task 3.2: `create_workspace`

**Files:**
- Modify: `crates/api-client/src/contract.rs` (append WorkspaceSelection + request types)
- Create: `crates/api-client/src/api/workspaces.rs`

- [ ] **Step 1: Write the failing test — `crates/api-client/src/api/workspaces.rs`**

```rust
use crate::contract::WorkspaceSelection;
use crate::http::ControlPlaneApi;
use httpmock::prelude::*;
use serde_json::json;

#[tokio::test]
async fn create_workspace_sends_policy_and_returns_selection() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/api/workspaces")
                .header("cookie", "cp_session=abc123")
                .body_contains(r#""name":"my workspace""#)
                .body_contains(r#""readOnly":true"#)
                .body_contains(r#""failurePolicy":"partial""#)
                .body_contains(r#""promptInjectionMode":"fail_run""#);
            then.status(201).json_body(json!({
                "requestId": "req_1",
                "workspaceId": "ws_1",
                "name": "my workspace",
                "ownerId": "@u:matrix.example.org",
                "status": "active",
                "createdAt": "2026-08-15T00:00:00.000Z"
            }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let workspace: WorkspaceSelection = client.create_workspace("  my workspace  ").await.unwrap();

    assert_eq!(workspace.workspace_id, "ws_1");
    assert_eq!(workspace.name, "my workspace");
    assert_eq!(workspace.status, "active");
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client create_workspace_sends_policy_and_returns_selection`
Expected: compile error — `cannot find method \`create_workspace\` for struct \`ControlPlaneApi\``.

- [ ] **Step 3: Write the implementation**

Append to `crates/api-client/src/contract.rs`:

```rust
/// POST /api/workspaces response (mirrors WorkspaceSelection).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSelection {
    pub workspace_id: String,
    pub name: String,
    pub owner_id: String,
    pub status: String,
    pub created_at: String,
}

/// POST /api/workspaces request body. The policy mirrors what the mobile app
/// always sends (read-only runs, partial failure, fail run on prompt injection).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkspaceRequest {
    pub name: String,
    pub policy: WorkspacePolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePolicy {
    pub read_only: bool,
    pub failure_policy: String,
    pub prompt_injection_mode: String,
}
```

Create `crates/api-client/src/api/workspaces.rs`:

```rust
use crate::contract::{CreateWorkspaceRequest, WorkspacePolicy, WorkspaceSelection};
use crate::error::ControlPlaneError;
use crate::http::ControlPlaneApi;
use serde_json::json;

impl ControlPlaneApi {
    /// POST /api/workspaces — create a workspace with the same policy the
    /// mobile app uses.
    pub async fn create_workspace(&self, name: &str) -> Result<WorkspaceSelection, ControlPlaneError> {
        let request = CreateWorkspaceRequest {
            name: name.trim().to_string(),
            policy: WorkspacePolicy {
                read_only: true,
                failure_policy: "partial".to_string(),
                prompt_injection_mode: "fail_run".to_string(),
            },
        };
        let body = json!(request);
        self.authenticated_request(reqwest::Method::POST, "/api/workspaces", Some(&body))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn create_workspace_sends_policy_and_returns_selection() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/api/workspaces")
                    .header("cookie", "cp_session=abc123")
                    .body_contains(r#""name":"my workspace""#)
                    .body_contains(r#""readOnly":true"#)
                    .body_contains(r#""failurePolicy":"partial""#)
                    .body_contains(r#""promptInjectionMode":"fail_run""#);
                then.status(201).json_body(json!({
                    "requestId": "req_1",
                    "workspaceId": "ws_1",
                    "name": "my workspace",
                    "ownerId": "@u:matrix.example.org",
                    "status": "active",
                    "createdAt": "2026-08-15T00:00:00.000Z"
                }));
            })
            .await;

        let client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let workspace: WorkspaceSelection = client.create_workspace("  my workspace  ").await.unwrap();

        assert_eq!(workspace.workspace_id, "ws_1");
        assert_eq!(workspace.name, "my workspace");
        assert_eq!(workspace.status, "active");
        mock.assert_async().await;
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p api-client`
Expected: `test result: ok. 16 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/contract.rs crates/api-client/src/api/workspaces.rs
git commit -m "feat(api-client): create_workspace with mobile policy"
```

### Task 3.3: `get_rooms`

**Files:**
- Modify: `crates/api-client/src/contract.rs` (append RoomSummary)
- Create: `crates/api-client/src/api/rooms.rs`

- [ ] **Step 1: Write the failing test — `crates/api-client/src/api/rooms.rs`**

```rust
use crate::contract::RoomSummary;
use crate::http::ControlPlaneApi;
use httpmock::prelude::*;
use serde_json::json;

#[tokio::test]
async fn get_rooms_returns_the_rooms_array() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/api/rooms").header("cookie", "cp_session=abc123");
            then.status(200).json_body(json!({
                "requestId": "req_1",
                "rooms": [
                    {
                        "roomId": "!a:matrix.example.org",
                        "homeserverUrl": "https://matrix.example.org",
                        "displayName": "Engineering",
                        "workspaceId": "ws_1"
                    },
                    {
                        "roomId": "!b:matrix.example.org",
                        "homeserverUrl": "https://matrix.example.org",
                        "displayName": null,
                        "workspaceId": null
                    }
                ]
            }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let rooms: Vec<RoomSummary> = client.get_rooms().await.unwrap();

    assert_eq!(rooms.len(), 2);
    assert_eq!(rooms[0].room_id, "!a:matrix.example.org");
    assert_eq!(rooms[0].display_name.as_deref(), Some("Engineering"));
    assert_eq!(rooms[0].workspace_id.as_deref(), Some("ws_1"));
    assert_eq!(rooms[1].workspace_id, None);
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client get_rooms_returns_the_rooms_array`
Expected: compile error — `cannot find method \`get_rooms\``.

- [ ] **Step 3: Write the implementation**

Append to `crates/api-client/src/contract.rs`:

```rust
/// GET /api/rooms item (mirrors RoomSummary).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomSummary {
    pub room_id: String,
    pub homeserver_url: String,
    pub display_name: Option<String>,
    pub workspace_id: Option<String>,
}
```

Create `crates/api-client/src/api/rooms.rs`:

```rust
use crate::contract::RoomSummary;
use crate::error::ControlPlaneError;
use crate::http::ControlPlaneApi;

impl ControlPlaneApi {
    /// GET /api/rooms — list joined rooms with their workspace bindings.
    pub async fn get_rooms(&self) -> Result<Vec<RoomSummary>, ControlPlaneError> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RoomsResponse {
            rooms: Vec<RoomSummary>,
        }
        let body: RoomsResponse =
            self.authenticated_request(reqwest::Method::GET, "/api/rooms", None).await?;
        Ok(body.rooms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use serde_json::json;

    #[tokio::test]
    async fn get_rooms_returns_the_rooms_array() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/api/rooms").header("cookie", "cp_session=abc123");
                then.status(200).json_body(json!({
                    "requestId": "req_1",
                    "rooms": [
                        {
                            "roomId": "!a:matrix.example.org",
                            "homeserverUrl": "https://matrix.example.org",
                            "displayName": "Engineering",
                            "workspaceId": "ws_1"
                        },
                        {
                            "roomId": "!b:matrix.example.org",
                            "homeserverUrl": "https://matrix.example.org",
                            "displayName": null,
                            "workspaceId": null
                        }
                    ]
                }));
            })
            .await;

        let client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let rooms: Vec<RoomSummary> = client.get_rooms().await.unwrap();

        assert_eq!(rooms.len(), 2);
        assert_eq!(rooms[0].room_id, "!a:matrix.example.org");
        assert_eq!(rooms[0].display_name.as_deref(), Some("Engineering"));
        assert_eq!(rooms[0].workspace_id.as_deref(), Some("ws_1"));
        assert_eq!(rooms[1].workspace_id, None);
        mock.assert_async().await;
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p api-client`
Expected: `test result: ok. 17 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/contract.rs crates/api-client/src/api/rooms.rs
git commit -m "feat(api-client): get_rooms"
```

### Task 3.4: `bind_room`

**Files:**
- Modify: `crates/api-client/src/contract.rs` (append RoomBinding + BindRoomRequest)
- Modify: `crates/api-client/src/api/rooms.rs`

- [ ] **Step 1: Write the failing test (append to the `tests` module in `crates/api-client/src/api/rooms.rs`)**

```rust
#[tokio::test]
async fn bind_room_posts_workspace_id_and_returns_binding() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/api/rooms/%21a%3Amatrix.example.org/binding")
                .body_contains(r#""workspaceId":"ws_1""#);
            then.status(200).json_body(json!({
                "roomId": "!a:matrix.example.org",
                "workspaceId": "ws_1"
            }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let binding: crate::contract::RoomBinding = client
        .bind_room("!a:matrix.example.org", "ws_1")
        .await
        .unwrap();

    assert_eq!(binding.room_id, "!a:matrix.example.org");
    assert_eq!(binding.workspace_id, "ws_1");
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client bind_room_posts_workspace_id_and_returns_binding`
Expected: compile error — `cannot find method \`bind_room\``.

- [ ] **Step 3: Write the implementation**

Append to `crates/api-client/src/contract.rs`:

```rust
/// POST /api/rooms/:roomId/binding response (mirrors RoomBinding).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomBinding {
    pub room_id: String,
    pub workspace_id: String,
}

/// POST /api/rooms/:roomId/binding request body.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindRoomRequest {
    pub workspace_id: String,
}
```

Append to `crates/api-client/src/api/rooms.rs` (add to the impl block):

```rust
impl ControlPlaneApi {
    /// POST /api/rooms/:roomId/binding — bind a room to a workspace.
    pub async fn bind_room(
        &self,
        room_id: &str,
        workspace_id: &str,
    ) -> Result<crate::contract::RoomBinding, ControlPlaneError> {
        use crate::contract::BindRoomRequest;
        let body = json!(BindRoomRequest {
            workspace_id: workspace_id.to_string(),
        });
        let path = format!("/api/rooms/{}/binding", crate::http::urlencode(room_id));
        self.authenticated_request(reqwest::Method::POST, &path, Some(&body))
            .await
    }
}
```

(This is a second impl block for `ControlPlaneApi` in the same file — that is legal Rust. The existing `get_rooms` impl block stays untouched.)

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p api-client`
Expected: `test result: ok. 18 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/contract.rs crates/api-client/src/api/rooms.rs
git commit -m "feat(api-client): bind_room"
```

### Task 3.5: `launch_run` (fresh idempotency key per launch)

**Files:**
- Modify: `crates/api-client/src/contract.rs` (append RunRequest/RunMode/GithubContext/RunResponse/RunStatus)
- Create: `crates/api-client/src/api/runs.rs`

- [ ] **Step 1: Write the failing test — `crates/api-client/src/api/runs.rs`**

```rust
use crate::contract::{RunMode, RunRequest, RunResponse, RunStatus};
use crate::http::ControlPlaneApi;
use httpmock::prelude::*;
use serde_json::json;

#[tokio::test]
async fn launch_run_sends_request_plus_idempotency_key() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/api/workspaces/ws_1/runs")
                .body_contains(r#""prompt":"Summarize the PRs""#)
                .body_contains(r#""mode":"parallel""#)
                .body_contains(r#""specialistIds":["pr-reader"]"#)
                .body_contains(r#""roomId":"!a:matrix.example.org""#)
                .body_contains(r#""idempotencyKey":"key_42""#);
            then.status(202).json_body(json!({
                "runId": "r1",
                "status": "queued",
                "roomId": "!a:matrix.example.org",
                "nextSequence": 1
            }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let request = RunRequest {
        prompt: "Summarize the PRs".to_string(),
        mode: RunMode::Parallel,
        specialist_ids: vec!["pr-reader".to_string()],
        room_id: Some("!a:matrix.example.org".to_string()),
        github_context: None,
    };
    let run: RunResponse = client.launch_run("ws_1", &request, "key_42").await.unwrap();

    assert_eq!(run.run_id, "r1");
    assert_eq!(run.status, RunStatus::Queued);
    assert_eq!(run.next_sequence, 1);
    mock.assert_async().await;
}

#[tokio::test]
async fn launch_run_omits_optional_fields_when_absent() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST).path("/api/workspaces/ws_1/runs");
            then.status(202).json_body(json!({
                "runId": "r2",
                "status": "queued",
                "nextSequence": 0
            }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let request = RunRequest {
        prompt: "hi".to_string(),
        mode: RunMode::Sequential,
        specialist_ids: vec!["repo-reader".to_string()],
        room_id: None,
        github_context: None,
    };
    let run: RunResponse = client.launch_run("ws_1", &request, "key_43").await.unwrap();
    assert_eq!(run.room_id, None);
    assert_eq!(run.status, RunStatus::Queued);
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client launch_run_sends_request_plus_idempotency_key`
Expected: compile error — `cannot find method \`launch_run\`` (and missing types `RunMode`, `RunResponse`, `RunStatus`).

- [ ] **Step 3: Write the implementation**

Append to `crates/api-client/src/contract.rs`:

```rust
/// Run execution mode (mirrors RunRequest.mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    Parallel,
    Sequential,
}

/// Launch body minus the idempotency key (mirrors RunRequest in packages/contracts/src/run.ts).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRequest {
    pub prompt: String,
    pub mode: RunMode,
    pub specialist_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_context: Option<GithubContext>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubContext {
    pub repository: String,
}

/// Run lifecycle status (mirrors RunResponse.status).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Cancelling,
    Completed,
    Failed,
    Cancelled,
    Partial,
}

/// POST /api/workspaces/:workspaceId/runs response (mirrors RunResponse).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResponse {
    pub run_id: String,
    pub status: RunStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_id: Option<String>,
    pub next_sequence: u64,
}
```

Create `crates/api-client/src/api/runs.rs`:

```rust
use crate::contract::{RunRequest, RunResponse};
use crate::error::ControlPlaneError;
use crate::http::{urlencode, ControlPlaneApi};
use serde_json::json;

impl ControlPlaneApi {
    /// POST /api/workspaces/:workspaceId/runs — launch a run. The caller
    /// always passes a fresh idempotency key (mirrors the mobile composer).
    pub async fn launch_run(
        &self,
        workspace_id: &str,
        request: &RunRequest,
        idempotency_key: &str,
    ) -> Result<RunResponse, ControlPlaneError> {
        let mut body = serde_json::to_value(request)
            .map_err(|e| ControlPlaneError::InvalidResponse(e.to_string()))?;
        if let Some(object) = body.as_object_mut() {
            object.insert(
                "idempotencyKey".to_string(),
                serde_json::Value::String(idempotency_key.to_string()),
            );
        }
        let path = format!("/api/workspaces/{}/runs", urlencode(workspace_id));
        self.authenticated_request(reqwest::Method::POST, &path, Some(&body))
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{RunMode, RunStatus};
    use httpmock::prelude::*;

    #[tokio::test]
    async fn launch_run_sends_request_plus_idempotency_key() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/api/workspaces/ws_1/runs")
                    .body_contains(r#""prompt":"Summarize the PRs""#)
                    .body_contains(r#""mode":"parallel""#)
                    .body_contains(r#""specialistIds":["pr-reader"]"#)
                    .body_contains(r#""roomId":"!a:matrix.example.org""#)
                    .body_contains(r#""idempotencyKey":"key_42""#);
                then.status(202).json_body(json!({
                    "runId": "r1",
                    "status": "queued",
                    "roomId": "!a:matrix.example.org",
                    "nextSequence": 1
                }));
            })
            .await;

        let client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let request = RunRequest {
            prompt: "Summarize the PRs".to_string(),
            mode: RunMode::Parallel,
            specialist_ids: vec!["pr-reader".to_string()],
            room_id: Some("!a:matrix.example.org".to_string()),
            github_context: None,
        };
        let run: RunResponse = client.launch_run("ws_1", &request, "key_42").await.unwrap();

        assert_eq!(run.run_id, "r1");
        assert_eq!(run.status, RunStatus::Queued);
        assert_eq!(run.next_sequence, 1);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn launch_run_omits_optional_fields_when_absent() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(POST).path("/api/workspaces/ws_1/runs");
                then.status(202).json_body(json!({
                    "runId": "r2",
                    "status": "queued",
                    "nextSequence": 0
                }));
            })
            .await;

        let client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let request = RunRequest {
            prompt: "hi".to_string(),
            mode: RunMode::Sequential,
            specialist_ids: vec!["repo-reader".to_string()],
            room_id: None,
            github_context: None,
        };
        let run: RunResponse = client.launch_run("ws_1", &request, "key_43").await.unwrap();
        assert_eq!(run.room_id, None);
        assert_eq!(run.status, RunStatus::Queued);
        mock.assert_async().await;
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p api-client`
Expected: `test result: ok. 20 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/contract.rs crates/api-client/src/api/runs.rs
git commit -m "feat(api-client): launch_run with idempotency key"
```

### Task 3.6: `cancel_run` and `get_run_matrix_deliveries`

**Files:**
- Modify: `crates/api-client/src/contract.rs` (append CancellationResponse/CancellationStatus/MatrixDeliveryStatus/MatrixDelivery/RunMatrixDeliveriesResponse)
- Modify: `crates/api-client/src/api/runs.rs`

- [ ] **Step 1: Write the failing tests (append to the `tests` module in `crates/api-client/src/api/runs.rs`)**

```rust
#[tokio::test]
async fn cancel_run_posts_and_returns_cancellation_requested() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST).path("/api/runs/r1/cancel");
            then.status(202).json_body(json!({
                "requestId": "req_1",
                "runId": "r1",
                "status": "cancellation_requested"
            }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let response = client.cancel_run("r1").await.unwrap();

    assert_eq!(response.run_id, "r1");
    assert_eq!(response.status, crate::contract::CancellationStatus::CancellationRequested);
    mock.assert_async().await;
}

#[tokio::test]
async fn get_run_matrix_deliveries_reads_authoritative_status() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/api/runs/r1");
            then.status(200).json_body(json!({
                "requestId": "req_1",
                "runId": "r1",
                "status": "running",
                "mode": "parallel",
                "workspaceId": "ws_1",
                "roomId": null,
                "specialists": [],
                "lastSequence": 5,
                "matrixDeliveries": [
                    { "sequence": 1, "status": "delivered" },
                    { "sequence": 2, "status": "pending" }
                ],
                "cancelRequestedAt": null
            }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let deliveries = client.get_run_matrix_deliveries("r1").await.unwrap();

    assert_eq!(deliveries.run_id, "r1");
    assert_eq!(deliveries.deliveries.len(), 2);
    assert_eq!(deliveries.deliveries[0].sequence, 1);
    assert_eq!(
        deliveries.deliveries[0].status,
        crate::contract::MatrixDeliveryStatus::Delivered
    );
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p api-client cancel_run_posts_and_returns_cancellation_requested`
Expected: compile error — `cannot find method \`cancel_run\``.

- [ ] **Step 3: Write the implementation**

Append to `crates/api-client/src/contract.rs`:

```rust
/// POST /api/runs/:runId/cancel response (mirrors CancellationResponse).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancellationResponse {
    pub run_id: String,
    pub status: CancellationStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationStatus {
    CancellationRequested,
}

/// Authoritative Matrix delivery status from GET /api/runs/:runId
/// (mirrors MatrixDeliveryStatus). Never inferred from the event stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatrixDeliveryStatus {
    Pending,
    Delivered,
    Failed,
    Dead,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixDelivery {
    pub sequence: u64,
    pub status: MatrixDeliveryStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunMatrixDeliveriesResponse {
    pub run_id: String,
    pub deliveries: Vec<MatrixDelivery>,
}
```

Append to `crates/api-client/src/api/runs.rs` (inside the existing impl block):

```rust
    /// POST /api/runs/:runId/cancel — request cancellation.
    pub async fn cancel_run(&self, run_id: &str) -> Result<crate::contract::CancellationResponse, ControlPlaneError> {
        let path = format!("/api/runs/{}/cancel", urlencode(run_id));
        self.authenticated_request(reqwest::Method::POST, &path, None)
            .await
    }

    /// GET /api/runs/:runId — read the authoritative Matrix delivery statuses.
    pub async fn get_run_matrix_deliveries(
        &self,
        run_id: &str,
    ) -> Result<crate::contract::RunMatrixDeliveriesResponse, ControlPlaneError> {
        use crate::contract::MatrixDelivery;
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RunDetailBody {
            run_id: String,
            matrix_deliveries: Vec<MatrixDelivery>,
        }
        let path = format!("/api/runs/{}", urlencode(run_id));
        let body: RunDetailBody =
            self.authenticated_request(reqwest::Method::GET, &path, None).await?;
        Ok(crate::contract::RunMatrixDeliveriesResponse {
            run_id: body.run_id,
            deliveries: body.matrix_deliveries,
        })
    }
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p api-client`
Expected: `test result: ok. 22 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/contract.rs crates/api-client/src/api/runs.rs
git commit -m "feat(api-client): cancel_run and get_run_matrix_deliveries"
```

### Task 3.7: `list_github_repositories`

**Files:**
- Modify: `crates/api-client/src/contract.rs` (append GithubPage + GithubRepositorySummary)
- Create: `crates/api-client/src/api/github.rs`

- [ ] **Step 1: Write the failing test — `crates/api-client/src/api/github.rs`**

```rust
use crate::contract::{GithubPage, GithubRepositorySummary};
use crate::http::ControlPlaneApi;
use httpmock::prelude::*;
use serde_json::json;

#[tokio::test]
async fn list_github_repositories_sends_query_params() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/api/github/repositories")
                .query_param("workspaceId", "ws_1")
                .query_param("installationId", "inst_9")
                .query_param("cursor", "p2");
            then.status(200).json_body(json!({
                "items": [
                    {
                        "id": 1,
                        "name": "repo",
                        "fullName": "octo/repo",
                        "owner": "octo",
                        "private": false,
                        "defaultBranch": "main",
                        "description": "A repo",
                        "htmlUrl": "https://github.com/octo/repo",
                        "archived": false
                    }
                ],
                "nextCursor": "p3"
            }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let page: GithubPage<GithubRepositorySummary> = client
        .list_github_repositories("ws_1", "inst_9", Some("p2"))
        .await
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].full_name, "octo/repo");
    assert_eq!(page.items[0].private, false);
    assert_eq!(page.next_cursor.as_deref(), Some("p3"));
    mock.assert_async().await;
}

#[tokio::test]
async fn list_github_repositories_without_cursor_omits_param() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/api/github/repositories")
                .query_param("workspaceId", "ws_1")
                .query_param("installationId", "inst_9");
            then.status(200).json_body(json!({ "items": [] }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let page: GithubPage<GithubRepositorySummary> = client
        .list_github_repositories("ws_1", "inst_9", None)
        .await
        .unwrap();
    assert!(page.items.is_empty());
    assert_eq!(page.next_cursor, None);
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client list_github_repositories_sends_query_params`
Expected: compile error — `cannot find method \`list_github_repositories\``.

- [ ] **Step 3: Write the implementation**

Append to `crates/api-client/src/contract.rs`:

```rust
/// Paginated GitHub read result (mirrors GithubPage<T>).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubPage<T> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Mirrors GithubRepositorySummary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepositorySummary {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub owner: String,
    pub private: bool,
    pub default_branch: String,
    pub description: Option<String>,
    pub html_url: String,
    pub archived: bool,
}
```

Create `crates/api-client/src/api/github.rs`:

```rust
use crate::contract::{GithubPage, GithubPullRequestSummary, GithubRepositorySummary};
use crate::error::ControlPlaneError;
use crate::http::{urlencode, ControlPlaneApi};

impl ControlPlaneApi {
    /// Build the shared `?workspaceId=..&installationId=..[&cursor=..]` query
    /// suffix for the GitHub read routes.
    fn github_read_path(
        &self,
        workspace_id: &str,
        installation_id: &str,
        suffix: &str,
        cursor: Option<&str>,
    ) -> String {
        let mut path = format!(
            "{suffix}?workspaceId={}&installationId={}",
            urlencode(workspace_id),
            urlencode(installation_id)
        );
        if let Some(cursor) = cursor {
            path.push_str(&format!("&cursor={}", urlencode(cursor)));
        }
        path
    }

    /// GET /api/github/repositories — list the installation's repositories.
    pub async fn list_github_repositories(
        &self,
        workspace_id: &str,
        installation_id: &str,
        cursor: Option<&str>,
    ) -> Result<GithubPage<GithubRepositorySummary>, ControlPlaneError> {
        let path = self.github_read_path(
            workspace_id,
            installation_id,
            "/api/github/repositories",
            cursor,
        );
        self.authenticated_request(reqwest::Method::GET, &path, None)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::GithubRepositorySummary;
    use httpmock::prelude::*;
    use serde_json::json;

    #[tokio::test]
    async fn list_github_repositories_sends_query_params() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/api/github/repositories")
                    .query_param("workspaceId", "ws_1")
                    .query_param("installationId", "inst_9")
                    .query_param("cursor", "p2");
                then.status(200).json_body(json!({
                    "items": [
                        {
                            "id": 1,
                            "name": "repo",
                            "fullName": "octo/repo",
                            "owner": "octo",
                            "private": false,
                            "defaultBranch": "main",
                            "description": "A repo",
                            "htmlUrl": "https://github.com/octo/repo",
                            "archived": false
                        }
                    ],
                    "nextCursor": "p3"
                }));
            })
            .await;

        let client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let page: GithubPage<GithubRepositorySummary> = client
            .list_github_repositories("ws_1", "inst_9", Some("p2"))
            .await
            .unwrap();

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].full_name, "octo/repo");
        assert_eq!(page.items[0].private, false);
        assert_eq!(page.next_cursor.as_deref(), Some("p3"));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn list_github_repositories_without_cursor_omits_param() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/api/github/repositories")
                    .query_param("workspaceId", "ws_1")
                    .query_param("installationId", "inst_9");
                then.status(200).json_body(json!({ "items": [] }));
            })
            .await;

        let client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let page: GithubPage<GithubRepositorySummary> = client
            .list_github_repositories("ws_1", "inst_9", None)
            .await
            .unwrap();
        assert!(page.items.is_empty());
        assert_eq!(page.next_cursor, None);
        mock.assert_async().await;
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p api-client`
Expected: `test result: ok. 24 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/contract.rs crates/api-client/src/api/github.rs
git commit -m "feat(api-client): list_github_repositories with cursor"
```

### Task 3.8: `list_github_issues` and `list_github_pull_requests`

**Files:**
- Modify: `crates/api-client/src/contract.rs` (append GithubIssueSummary + GithubPullRequestSummary)
- Modify: `crates/api-client/src/api/github.rs`

- [ ] **Step 1: Write the failing tests (append to the `tests` module in `crates/api-client/src/api/github.rs`)**

```rust
#[tokio::test]
async fn list_github_issues_uses_owner_repo_path_and_cursor() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/api/github/repositories/octo/repo/issues")
                .query_param("workspaceId", "ws_1")
                .query_param("installationId", "inst_9")
                .query_param("cursor", "p2");
            then.status(200).json_body(json!({
                "items": [
                    {
                        "id": 11,
                        "number": 42,
                        "title": "Fix the bug",
                        "state": "open",
                        "author": "octo",
                        "labels": ["bug"],
                        "htmlUrl": "https://github.com/octo/repo/issues/42",
                        "createdAt": "2026-08-01T00:00:00.000Z",
                        "updatedAt": "2026-08-02T00:00:00.000Z"
                    }
                ]
            }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let page: GithubPage<crate::contract::GithubIssueSummary> = client
        .list_github_issues("ws_1", "inst_9", "octo", "repo", Some("p2"))
        .await
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].number, 42);
    assert_eq!(page.items[0].title, "Fix the bug");
    assert_eq!(page.items[0].labels, vec!["bug"]);
    assert_eq!(page.next_cursor, None);
    mock.assert_async().await;
}

#[tokio::test]
async fn list_github_pull_requests_parses_draft_and_head_base() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/api/github/repositories/octo/repo/pulls")
                .query_param("workspaceId", "ws_1")
                .query_param("installationId", "inst_9");
            then.status(200).json_body(json!({
                "items": [
                    {
                        "id": 22,
                        "number": 7,
                        "title": "Add docs",
                        "state": "open",
                        "draft": true,
                        "author": null,
                        "head": "octo:docs",
                        "base": "main",
                        "htmlUrl": "https://github.com/octo/repo/pull/7",
                        "createdAt": "2026-08-01T00:00:00.000Z",
                        "updatedAt": "2026-08-02T00:00:00.000Z"
                    }
                ]
            }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let page: GithubPage<crate::contract::GithubPullRequestSummary> = client
        .list_github_pull_requests("ws_1", "inst_9", "octo", "repo", None)
        .await
        .unwrap();

    assert_eq!(page.items[0].draft, true);
    assert_eq!(page.items[0].author, None);
    assert_eq!(page.items[0].head, "octo:docs");
    assert_eq!(page.items[0].base, "main");
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client list_github_issues_uses_owner_repo_path_and_cursor`
Expected: compile error — `cannot find method \`list_github_issues\``.

- [ ] **Step 3: Write the implementation**

Append to `crates/api-client/src/contract.rs`:

```rust
/// Mirrors GithubIssueSummary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubIssueSummary {
    pub id: u64,
    pub number: u64,
    pub title: String,
    pub state: String,
    pub author: Option<String>,
    pub labels: Vec<String>,
    pub html_url: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Mirrors GithubPullRequestSummary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubPullRequestSummary {
    pub id: u64,
    pub number: u64,
    pub title: String,
    pub state: String,
    pub draft: bool,
    pub author: Option<String>,
    pub head: String,
    pub base: String,
    pub html_url: String,
    pub created_at: String,
    pub updated_at: String,
}
```

Append to `crates/api-client/src/api/github.rs` (inside the existing impl block):

```rust
    /// GET /api/github/repositories/:owner/:repo/issues
    pub async fn list_github_issues(
        &self,
        workspace_id: &str,
        installation_id: &str,
        owner: &str,
        repo: &str,
        cursor: Option<&str>,
    ) -> Result<GithubPage<crate::contract::GithubIssueSummary>, ControlPlaneError> {
        let suffix = format!(
            "/api/github/repositories/{}/{}/issues",
            urlencode(owner),
            urlencode(repo)
        );
        let path = self.github_read_path(workspace_id, installation_id, &suffix, cursor);
        self.authenticated_request(reqwest::Method::GET, &path, None)
            .await
    }

    /// GET /api/github/repositories/:owner/:repo/pulls
    pub async fn list_github_pull_requests(
        &self,
        workspace_id: &str,
        installation_id: &str,
        owner: &str,
        repo: &str,
        cursor: Option<&str>,
    ) -> Result<GithubPage<GithubPullRequestSummary>, ControlPlaneError> {
        let suffix = format!(
            "/api/github/repositories/{}/{}/pulls",
            urlencode(owner),
            urlencode(repo)
        );
        let path = self.github_read_path(workspace_id, installation_id, &suffix, cursor);
        self.authenticated_request(reqwest::Method::GET, &path, None)
            .await
    }
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p api-client`
Expected: `test result: ok. 26 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/contract.rs crates/api-client/src/api/github.rs
git commit -m "feat(api-client): list_github_issues and list_github_pull_requests"
```

### Task 3.9: `request_github_write_grant`

**Files:**
- Modify: `crates/api-client/src/contract.rs` (append GithubWriteScope/GrantStatus/GithubWriteGrantResult/CreateGrantRequest)
- Modify: `crates/api-client/src/api/github.rs`

- [ ] **Step 1: Write the failing test (append to the `tests` module in `crates/api-client/src/api/github.rs`)**

```rust
#[tokio::test]
async fn request_github_write_grant_posts_repository_and_scope() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/api/workspaces/ws_1/github-grants")
                .body_contains(r#""repository":"octo/repo""#)
                .body_contains(r#""scope":"issues:write""#);
            then.status(201).json_body(json!({
                "grantId": "gr_1",
                "status": "pending",
                "repository": "octo/repo",
                "scope": "issues:write"
            }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let result = client
        .request_github_write_grant("ws_1", "octo/repo", crate::contract::GithubWriteScope::IssuesWrite)
        .await
        .unwrap();

    assert_eq!(result.grant_id, "gr_1");
    assert_eq!(result.status, crate::contract::GrantStatus::Pending);
    assert_eq!(result.scope, crate::contract::GithubWriteScope::IssuesWrite);
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client request_github_write_grant_posts_repository_and_scope`
Expected: compile error — `cannot find method \`request_github_write_grant\``.

- [ ] **Step 3: Write the implementation**

Append to `crates/api-client/src/contract.rs`:

```rust
/// GitHub write scope (mirrors GithubWriteScope).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GithubWriteScope {
    IssuesWrite,
    PullRequestsWrite,
}

/// Grant lifecycle status (mirrors GithubWriteGrantResult.status).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantStatus {
    Pending,
    Approved,
    Revoked,
}

/// POST /api/workspaces/:workspaceId/github-grants response
/// (mirrors GithubWriteGrantResult).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubWriteGrantResult {
    pub grant_id: String,
    pub status: GrantStatus,
    pub repository: String,
    pub scope: GithubWriteScope,
}

/// POST /api/workspaces/:workspaceId/github-grants request body.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGrantRequest {
    pub repository: String,
    pub scope: GithubWriteScope,
}
```

Append to `crates/api-client/src/api/github.rs` (inside the existing impl block):

```rust
    /// POST /api/workspaces/:workspaceId/github-grants — request a separate
    /// repository+scope write grant (Phase B read auth never implies write).
    pub async fn request_github_write_grant(
        &self,
        workspace_id: &str,
        repository: &str,
        scope: crate::contract::GithubWriteScope,
    ) -> Result<crate::contract::GithubWriteGrantResult, ControlPlaneError> {
        use crate::contract::CreateGrantRequest;
        let body = json!(CreateGrantRequest {
            repository: repository.to_string(),
            scope,
        });
        let path = format!("/api/workspaces/{}/github-grants", urlencode(workspace_id));
        self.authenticated_request(reqwest::Method::POST, &path, Some(&body))
            .await
    }
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p api-client`
Expected: `test result: ok. 27 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/contract.rs crates/api-client/src/api/github.rs
git commit -m "feat(api-client): request_github_write_grant"
```

### Task 3.10: `create_run_approval`

**Files:**
- Modify: `crates/api-client/src/contract.rs` (append ApprovalStatus/RunApprovalResult/ApprovalType/ApprovalDecision/CreateApprovalRequest)
- Modify: `crates/api-client/src/api/runs.rs`

- [ ] **Step 1: Write the failing test (append to the `tests` module in `crates/api-client/src/api/runs.rs`)**

```rust
#[tokio::test]
async fn create_run_approval_posts_exact_confirmation() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/api/runs/r1/approvals")
                .body_contains(r#""approvalType":"github_mutation""#)
                .body_contains(r#""scope":"issues:write""#)
                .body_contains(r#""decision":"approved""#)
                .body_contains(r#""confirmationText":"I confirm create issue on octo/repo (issues:write)""#)
                .body_contains(r#""commandHash":"22a9632d51b690e300e3ef7fb397048392bc84a388c4ef68beb0d42202815fd8""#);
            then.status(200).json_body(json!({
                "approvalId": "apr_1",
                "status": "approved",
                "expiresAt": "2026-08-15T01:00:00.000Z",
                "scope": "issues:write"
            }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let request = crate::contract::CreateApprovalRequest {
        approval_type: crate::contract::ApprovalType::GithubMutation,
        scope: crate::contract::GithubWriteScope::IssuesWrite,
        decision: crate::contract::ApprovalDecision::Approved,
        confirmation_text: "I confirm create issue on octo/repo (issues:write)".to_string(),
        command_hash: "22a9632d51b690e300e3ef7fb397048392bc84a388c4ef68beb0d42202815fd8".to_string(),
    };
    let result = client.create_run_approval("r1", &request).await.unwrap();

    assert_eq!(result.approval_id, "apr_1");
    assert_eq!(result.status, crate::contract::ApprovalStatus::Approved);
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client create_run_approval_posts_exact_confirmation`
Expected: compile error — `cannot find method \`create_run_approval\``.

- [ ] **Step 3: Write the implementation**

Append to `crates/api-client/src/contract.rs`:

```rust
/// Approval lifecycle status (mirrors RunApprovalResult.status).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Approved,
    Denied,
}

/// POST /api/runs/:runId/approvals response (mirrors RunApprovalResult).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunApprovalResult {
    pub approval_id: String,
    pub status: ApprovalStatus,
    pub expires_at: String,
    pub scope: GithubWriteScope,
}

/// POST /api/runs/:runId/approvals request body (mirrors the mobile
/// createRunApproval input: approvalType literal + exact confirmation text).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateApprovalRequest {
    pub approval_type: ApprovalType,
    pub scope: GithubWriteScope,
    pub decision: ApprovalDecision,
    pub confirmation_text: String,
    pub command_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalType {
    GithubMutation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approved,
    Denied,
}
```

Append to `crates/api-client/src/api/runs.rs` (inside the existing impl block):

```rust
    /// POST /api/runs/:runId/approvals — record the explicit human approval.
    /// Only ever called from the explicit mutation confirmation action.
    pub async fn create_run_approval(
        &self,
        run_id: &str,
        request: &crate::contract::CreateApprovalRequest,
    ) -> Result<crate::contract::RunApprovalResult, ControlPlaneError> {
        let body = serde_json::to_value(request)
            .map_err(|e| ControlPlaneError::InvalidResponse(e.to_string()))?;
        let path = format!("/api/runs/{}/approvals", urlencode(run_id));
        self.authenticated_request(reqwest::Method::POST, &path, Some(&body))
            .await
    }
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p api-client`
Expected: `test result: ok. 28 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/contract.rs crates/api-client/src/api/runs.rs
git commit -m "feat(api-client): create_run_approval with exact confirmation"
```

### Task 3.11: `enqueue_github_mutation` (200 replay vs 202 new)

**Files:**
- Modify: `crates/api-client/src/contract.rs` (append GithubMutationOperation/MutationStatus/GithubMutationResult/EnqueueMutationRequest)
- Modify: `crates/api-client/src/api/github.rs`

- [ ] **Step 1: Write the failing tests (append to the `tests` module in `crates/api-client/src/api/github.rs`)**

```rust
#[tokio::test]
async fn enqueue_github_mutation_202_is_new_command() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST)
                .path("/api/workspaces/ws_1/github/mutations")
                .body_contains(r#""idempotencyKey":"key_1""#)
                .body_contains(r#""approvalId":"apr_1""#)
                .body_contains(r#""repository":"octo/repo""#)
                .body_contains(r#""runId":"r1""#)
                .body_contains(r#""operation":"create_issue""#)
                .body_contains(r#""title":"Fix the bug""#);
            then.status(202).json_body(json!({
                "commandId": "cmd_1",
                "status": "queued"
            }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let request = crate::contract::EnqueueMutationRequest {
        idempotency_key: "key_1".to_string(),
        approval_id: "apr_1".to_string(),
        repository: "octo/repo".to_string(),
        run_id: Some("r1".to_string()),
        operation: crate::contract::GithubMutationOperation::CreateIssue,
        arguments: serde_json::json!({ "title": "Fix the bug" }),
    };
    let result = client.enqueue_github_mutation("ws_1", &request).await.unwrap();

    assert_eq!(result.command_id, "cmd_1");
    assert_eq!(result.status, crate::contract::MutationStatus::Queued);
    assert!(!result.replayed);
    mock.assert_async().await;
}

#[tokio::test]
async fn enqueue_github_mutation_200_is_idempotent_replay() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(POST).path("/api/workspaces/ws_1/github/mutations");
            then.status(200).json_body(json!({
                "commandId": "cmd_1",
                "status": "completed"
            }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let request = crate::contract::EnqueueMutationRequest {
        idempotency_key: "key_1".to_string(),
        approval_id: "apr_1".to_string(),
        repository: "octo/repo".to_string(),
        run_id: None,
        operation: crate::contract::GithubMutationOperation::CreateIssue,
        arguments: serde_json::json!({ "title": "Fix the bug" }),
    };
    let result = client.enqueue_github_mutation("ws_1", &request).await.unwrap();

    assert_eq!(result.status, crate::contract::MutationStatus::Completed);
    assert!(result.replayed);
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client enqueue_github_mutation_202_is_new_command`
Expected: compile error — `cannot find method \`enqueue_github_mutation\``.

- [ ] **Step 3: Write the implementation**

Append to `crates/api-client/src/contract.rs`:

```rust
/// GitHub mutation operations (mirrors GithubMutationOperation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GithubMutationOperation {
    CreateIssue,
    UpdateIssue,
    CommentIssue,
    CreatePrComment,
}

/// Command lifecycle status (mirrors GithubMutationResult.status).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationStatus {
    Queued,
    Completed,
    Failed,
}

/// POST /api/workspaces/:workspaceId/github/mutations response
/// (mirrors GithubMutationResult; `replayed` is derived from the status code:
/// 200 = idempotent replay, 202 = newly queued).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubMutationResult {
    pub command_id: String,
    pub status: MutationStatus,
    pub replayed: bool,
}

/// POST /api/workspaces/:workspaceId/github/mutations request body.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueMutationRequest {
    pub idempotency_key: String,
    pub approval_id: String,
    pub repository: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub operation: GithubMutationOperation,
    pub arguments: serde_json::Value,
}
```

Append to `crates/api-client/src/api/github.rs` (inside the existing impl block):

```rust
    /// POST /api/workspaces/:workspaceId/github/mutations — enqueue an
    /// approval-gated, idempotent mutation. 202 = newly queued, 200 = replay
    /// of the same idempotency key (mirrors the mobile's replayed flag).
    pub async fn enqueue_github_mutation(
        &self,
        workspace_id: &str,
        request: &crate::contract::EnqueueMutationRequest,
    ) -> Result<crate::contract::GithubMutationResult, ControlPlaneError> {
        use crate::contract::{GithubMutationResult, MutationStatus};
        let body = serde_json::to_value(request)
            .map_err(|e| ControlPlaneError::InvalidResponse(e.to_string()))?;
        let path = format!("/api/workspaces/{}/github/mutations", urlencode(workspace_id));
        let (status, value) = self
            .authenticated_response(reqwest::Method::POST, &path, Some(&body))
            .await?;
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct MutationBody {
            command_id: String,
            status: MutationStatus,
        }
        let parsed: MutationBody = serde_json::from_value(value)
            .map_err(|e| ControlPlaneError::InvalidResponse(e.to_string()))?;
        Ok(GithubMutationResult {
            command_id: parsed.command_id,
            status: parsed.status,
            replayed: status == reqwest::StatusCode::OK,
        })
    }
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p api-client`
Expected: `test result: ok. 30 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/contract.rs crates/api-client/src/api/github.rs
git commit -m "feat(api-client): enqueue_github_mutation with replay detection"
```

### Task 3.12: `list_audit_records`

**Files:**
- Modify: `crates/api-client/src/contract.rs` (append AuditRecordItem)
- Create: `crates/api-client/src/api/audit.rs`

- [ ] **Step 1: Write the failing test — `crates/api-client/src/api/audit.rs`**

```rust
use crate::contract::{AuditRecordItem, GithubPage};
use crate::http::ControlPlaneApi;
use httpmock::prelude::*;
use serde_json::json;

#[tokio::test]
async fn list_audit_records_parses_items_and_cursor() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/api/workspaces/ws_1/audit")
                .query_param("cursor", "p2");
            then.status(200).json_body(json!({
                "requestId": "req_1",
                "items": [
                    {
                        "id": "au_1",
                        "actorMatrixId": "@u:matrix.example.org",
                        "scope": "issues:write",
                        "repository": "octo/repo",
                        "operation": "create_issue",
                        "approvalId": "apr_1",
                        "commandId": "cmd_1",
                        "outcome": "completed",
                        "details": { "title": "Fix the bug" },
                        "createdAt": "2026-08-15T00:00:00.000Z"
                    }
                ],
                "nextCursor": "p3"
            }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let page: GithubPage<AuditRecordItem> = client.list_audit_records("ws_1", Some("p2")).await.unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, "au_1");
    assert_eq!(page.items[0].operation.as_deref(), Some("create_issue"));
    assert_eq!(page.items[0].details["title"], "Fix the bug");
    assert_eq!(page.next_cursor.as_deref(), Some("p3"));
    mock.assert_async().await;
}

#[tokio::test]
async fn list_audit_records_without_cursor_has_no_query() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/api/workspaces/ws_1/audit");
            then.status(200).json_body(json!({ "requestId": "req_1", "items": [] }));
        })
        .await;

    let client = ControlPlaneApi::new(server.base_url()).unwrap();
    client.set_cookie(Some("cp_session=abc123".to_string()));
    let page: GithubPage<AuditRecordItem> = client.list_audit_records("ws_1", None).await.unwrap();
    assert!(page.items.is_empty());
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client list_audit_records_parses_items_and_cursor`
Expected: compile error — `cannot find method \`list_audit_records\``.

- [ ] **Step 3: Write the implementation**

Append to `crates/api-client/src/contract.rs`:

```rust
/// Append-only audit trail item (mirrors AuditRecordItem).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditRecordItem {
    pub id: String,
    pub actor_matrix_id: Option<String>,
    pub scope: Option<String>,
    pub repository: Option<String>,
    pub operation: Option<String>,
    pub approval_id: Option<String>,
    pub command_id: Option<String>,
    pub outcome: String,
    pub details: serde_json::Value,
    pub created_at: String,
}
```

Create `crates/api-client/src/api/audit.rs`:

```rust
use crate::contract::{AuditRecordItem, GithubPage};
use crate::error::ControlPlaneError;
use crate::http::{urlencode, ControlPlaneApi};

impl ControlPlaneApi {
    /// GET /api/workspaces/:workspaceId/audit — keyset-paginated audit trail.
    pub async fn list_audit_records(
        &self,
        workspace_id: &str,
        cursor: Option<&str>,
    ) -> Result<GithubPage<AuditRecordItem>, ControlPlaneError> {
        let mut path = format!("/api/workspaces/{}/audit", urlencode(workspace_id));
        if let Some(cursor) = cursor {
            path.push_str(&format!("?cursor={}", urlencode(cursor)));
        }
        self.authenticated_request(reqwest::Method::GET, &path, None)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use serde_json::json;

    #[tokio::test]
    async fn list_audit_records_parses_items_and_cursor() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/api/workspaces/ws_1/audit")
                    .query_param("cursor", "p2");
                then.status(200).json_body(json!({
                    "requestId": "req_1",
                    "items": [
                        {
                            "id": "au_1",
                            "actorMatrixId": "@u:matrix.example.org",
                            "scope": "issues:write",
                            "repository": "octo/repo",
                            "operation": "create_issue",
                            "approvalId": "apr_1",
                            "commandId": "cmd_1",
                            "outcome": "completed",
                            "details": { "title": "Fix the bug" },
                            "createdAt": "2026-08-15T00:00:00.000Z"
                        }
                    ],
                    "nextCursor": "p3"
                }));
            })
            .await;

        let client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let page: GithubPage<AuditRecordItem> = client.list_audit_records("ws_1", Some("p2")).await.unwrap();

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, "au_1");
        assert_eq!(page.items[0].operation.as_deref(), Some("create_issue"));
        assert_eq!(page.items[0].details["title"], "Fix the bug");
        assert_eq!(page.next_cursor.as_deref(), Some("p3"));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn list_audit_records_without_cursor_has_no_query() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/api/workspaces/ws_1/audit");
                then.status(200).json_body(json!({ "requestId": "req_1", "items": [] }));
            })
            .await;

        let client = ControlPlaneApi::new(server.base_url()).unwrap();
        client.set_cookie(Some("cp_session=abc123".to_string()));
        let page: GithubPage<AuditRecordItem> = client.list_audit_records("ws_1", None).await.unwrap();
        assert!(page.items.is_empty());
        mock.assert_async().await;
    }
}
```

- [ ] **Step 4: Run the full api-client suite**

Run: `cargo test -p api-client`
Expected: `test result: ok. 32 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/contract.rs crates/api-client/src/api/audit.rs
git commit -m "feat(api-client): list_audit_records"
```

---

# Group 4: api-client — SSE event stream

The SSE module mirrors `apps/mobile/src/api/run-events.ts` + `packages/contracts/src/events.ts`: frame parsing, strict event validation, resume-from-last-sequence, terminal-event dedupe, and reconnect with backoff. All parsing/validation is pure and unit-tested; the `EventStream` connection behavior is tested against httpmock.

### Task 4.1: `parse_sse_frame`

**Files:**
- Create: `crates/api-client/src/sse.rs`
- Modify: `crates/api-client/src/lib.rs`

- [ ] **Step 1: Write the failing test (in `crates/api-client/src/sse.rs`, `#[cfg(test)] mod tests`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_id_event_and_data_lines() {
        let frame = parse_sse_frame("id: 3\nevent: specialist.started\ndata: {\"a\":1}\n\n").unwrap();
        assert_eq!(frame.id.as_deref(), Some("3"));
        assert_eq!(frame.event.as_deref(), Some("specialist.started"));
        assert_eq!(frame.data, "{\"a\":1}");
    }

    #[test]
    fn skips_comment_lines_and_blank_fields() {
        let frame = parse_sse_frame(": heartbeat\nid: 2\nevent: run.started\ndata: x\n\n").unwrap();
        assert_eq!(frame.id.as_deref(), Some("2"));
        assert_eq!(frame.data, "x");
    }

    #[test]
    fn handles_windows_line_endings_and_missing_field_value() {
        let frame = parse_sse_frame("id: 1\r\nevent: run.started\r\ndata: y\r\n\r\n").unwrap();
        assert_eq!(frame.id.as_deref(), Some("1"));
        assert_eq!(frame.data, "y");

        let bare = parse_sse_frame("id: 1\n\n").unwrap();
        assert_eq!(bare.id.as_deref(), Some("1"));
        assert_eq!(bare.data, "");
    }

    #[test]
    fn joins_multiple_data_lines_with_newline() {
        let frame = parse_sse_frame("id: 5\ndata: line1\ndata: line2\n\n").unwrap();
        assert_eq!(frame.data, "line1\nline2");
    }

    #[test]
    fn returns_none_when_no_data_field() {
        assert!(parse_sse_frame(": heartbeat\n\n").is_none());
        assert!(parse_sse_frame("").is_none());
    }
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client parses_id_event_and_data_lines`
Expected: compile error — `cannot find function \`parse_sse_frame\``.

- [ ] **Step 3: Write the implementation — top of `crates/api-client/src/sse.rs`**

```rust
use crate::contract::{EventVisibility, RunEvent, RunEventType};
use crate::error::ControlPlaneError;
use crate::http::urlencode;
use futures_util::StreamExt;
use reqwest::StatusCode;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// A single parsed SSE frame. `data` is the joined `data:` payload.
#[derive(Debug, Clone, PartialEq)]
pub struct SseFrame {
    pub id: Option<String>,
    pub event: Option<String>,
    pub data: String,
}

/// Parse one SSE frame (a block of lines ending in a blank line). Comment
/// lines (`: ...`) are skipped; frames without a `data:` field are ignored
/// (heartbeats). Mirrors `parseSseFrame` in apps/mobile/src/api/run-events.ts.
pub fn parse_sse_frame(raw: &str) -> Option<SseFrame> {
    let mut id = None;
    let mut event = None;
    let mut data = Vec::new();
    for raw_line in raw.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line, ""),
        };
        match field {
            "id" if !value.contains('\0') => id = Some(value.to_string()),
            "event" => event = Some(value.to_string()),
            "data" => data.push(value.to_string()),
            _ => {}
        }
    }
    if data.is_empty() {
        return None;
    }
    Some(SseFrame {
        id,
        event,
        data: data.join("\n"),
    })
}
```

(Keep the `#[cfg(test)] mod tests` from Step 1 below it.)

- [ ] **Step 4: Wire the module into `crates/api-client/src/lib.rs` and run the tests**

```rust
pub mod sse;
```

Run: `cargo test -p api-client`
Expected: `test result: ok. 37 passed; 0 failed; ...` (the contract types referenced by `sse.rs` do not exist yet — this task's Step 3 references `RunEvent`, `EventVisibility`, `RunEventType`, which are created in Task 4.2. To keep this task self-contained and green, temporarily comment out the `use crate::contract::{...}` line and the methods below `parse_sse_frame` that reference them (Task 4.3+ content) — only `parse_sse_frame` + its tests are in scope for this task. The next task adds the types and uncomments.)

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/sse.rs crates/api-client/src/lib.rs
git commit -m "feat(api-client): parse_sse_frame"
```

### Task 4.2: `RunEvent`, `RunEventType`, `EventVisibility` + `RunEvent::from_sse_frame` validation

**Files:**
- Modify: `crates/api-client/src/contract.rs` (append RunEvent + RunEventType + EventVisibility)
- Modify: `crates/api-client/src/sse.rs` (uncomment contract usage, add `from_sse_frame` + `validate` + `as_str`)

- [ ] **Step 1: Write the failing tests (append to the `tests` module in `crates/api-client/src/sse.rs`)**

```rust
#[test]
fn run_event_round_trips_camel_case_json() {
    let json = serde_json::json!({
        "id": "ev_1",
        "runId": "r1",
        "sequence": 1,
        "type": "run.started",
        "version": 1,
        "occurredAt": "2026-08-15T00:00:00.000Z",
        "visibility": "room_and_owner",
        "payload": { "note": "x" }
    });
    let event: RunEvent = serde_json::from_value(json).unwrap();
    assert_eq!(event.id, "ev_1");
    assert_eq!(event.run_id, "r1");
    assert_eq!(event.sequence, 1);
    assert_eq!(event.event_type, RunEventType::RunStarted);
    assert_eq!(event.version, 1);
    assert_eq!(event.visibility, EventVisibility::RoomAndOwner);
    assert_eq!(event.event_type.as_str(), "run.started");
}

#[test]
fn unknown_event_type_is_rejected() {
    let json = serde_json::json!({
        "id": "ev_1",
        "runId": "r1",
        "sequence": 1,
        "type": "run.teleported",
        "version": 1,
        "occurredAt": "2026-08-15T00:00:00.000Z",
        "visibility": "room_and_owner",
        "payload": {}
    });
    assert!(serde_json::from_value::<RunEvent>(json).is_err());
}

#[test]
fn all_contract_event_types_parse() {
    let types = [
        "run.queued", "run.started", "specialist.started", "specialist.progress",
        "specialist.completed", "specialist.failed", "run.partial", "run.checkpointed",
        "run.retry_scheduled", "run.cancellation_requested", "run.cancelled",
        "run.completed", "run.failed", "approval.requested", "approval.recorded",
        "mutation.queued", "mutation.completed", "mutation.failed",
    ];
    for name in types {
        let json = serde_json::json!({
            "id": "ev_1", "runId": "r1", "sequence": 1, "type": name, "version": 1,
            "occurredAt": "2026-08-15T00:00:00.000Z", "visibility": "room_and_owner", "payload": {}
        });
        let event: RunEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.event_type.as_str(), name);
    }
}

#[test]
fn from_sse_frame_accepts_valid_frame_for_expected_run() {
    let frame = SseFrame {
        id: Some("1".to_string()),
        event: Some("run.started".to_string()),
        data: r#"{"id":"ev_1","runId":"r1","sequence":1,"type":"run.started","version":1,"occurredAt":"2026-08-15T00:00:00.000Z","visibility":"room_and_owner","payload":{"note":"x"}}"#.to_string(),
    };
    let event = RunEvent::from_sse_frame(&frame, "r1").expect("valid event");
    assert_eq!(event.sequence, 1);
    assert_eq!(event.event_type, RunEventType::RunStarted);
}

#[test]
fn from_sse_frame_rejects_wrong_run_and_mismatched_event_name() {
    let data = r#"{"id":"ev_1","runId":"r1","sequence":1,"type":"run.started","version":1,"occurredAt":"2026-08-15T00:00:00.000Z","visibility":"room_and_owner","payload":{}}"#;
    let wrong_run = SseFrame {
        id: Some("1".to_string()),
        event: Some("run.started".to_string()),
        data: data.to_string(),
    };
    assert!(RunEvent::from_sse_frame(&wrong_run, "r2").is_none());

    let mismatched = SseFrame {
        id: Some("1".to_string()),
        event: Some("run.completed".to_string()),
        data: data.to_string(),
    };
    assert!(RunEvent::from_sse_frame(&mismatched, "r1").is_none());
}

#[test]
fn from_sse_frame_rejects_malformed_wire_data() {
    let cases = [
        SseFrame { id: Some("abc".to_string()), event: Some("run.started".to_string()), data: "{}".to_string() },
        SseFrame { id: Some("1".to_string()), event: None, data: "{}".to_string() },
        SseFrame { id: Some("1".to_string()), event: Some("run.started".to_string()), data: "not json".to_string() },
        SseFrame { id: Some("1".to_string()), event: Some("run.started".to_string()), data: r#"{"id":"ev_1","runId":"r1","sequence":9,"type":"run.started","version":1,"occurredAt":"2026-08-15T00:00:00.000Z","visibility":"room_and_owner","payload":{}}"#.to_string() },
        SseFrame { id: Some("1".to_string()), event: Some("run.started".to_string()), data: r#"{"id":"","runId":"r1","sequence":1,"type":"run.started","version":1,"occurredAt":"2026-08-15T00:00:00.000Z","visibility":"room_and_owner","payload":{}}"#.to_string() },
        SseFrame { id: Some("1".to_string()), event: Some("run.started".to_string()), data: r#"{"id":"ev_1","runId":"r1","sequence":1,"type":"run.started","version":1,"occurredAt":"2026-08-15T00:00:00.000Z","visibility":"room_and_owner","payload":"not-an-object"}"#.to_string() },
    ];
    for frame in cases {
        assert!(RunEvent::from_sse_frame(&frame, "r1").is_none(), "expected rejection for {frame:?}");
    }
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client run_event_round_trips_camel_case_json`
Expected: compile error — `cannot find type \`RunEvent\`` (and `EventVisibility`, `RunEventType`).

- [ ] **Step 3: Write the implementation**

Append to `crates/api-client/src/contract.rs`:

```rust
/// All allowed event types (mirrors RUN_EVENT_TYPES in
/// packages/contracts/src/events.ts — the wire names contain dots, so each
/// variant uses an explicit rename).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunEventType {
    #[serde(rename = "run.queued")]
    RunQueued,
    #[serde(rename = "run.started")]
    RunStarted,
    #[serde(rename = "specialist.started")]
    SpecialistStarted,
    #[serde(rename = "specialist.progress")]
    SpecialistProgress,
    #[serde(rename = "specialist.completed")]
    SpecialistCompleted,
    #[serde(rename = "specialist.failed")]
    SpecialistFailed,
    #[serde(rename = "run.partial")]
    RunPartial,
    #[serde(rename = "run.checkpointed")]
    RunCheckpointed,
    #[serde(rename = "run.retry_scheduled")]
    RunRetryScheduled,
    #[serde(rename = "run.cancellation_requested")]
    RunCancellationRequested,
    #[serde(rename = "run.cancelled")]
    RunCancelled,
    #[serde(rename = "run.completed")]
    RunCompleted,
    #[serde(rename = "run.failed")]
    RunFailed,
    #[serde(rename = "approval.requested")]
    ApprovalRequested,
    #[serde(rename = "approval.recorded")]
    ApprovalRecorded,
    #[serde(rename = "mutation.queued")]
    MutationQueued,
    #[serde(rename = "mutation.completed")]
    MutationCompleted,
    #[serde(rename = "mutation.failed")]
    MutationFailed,
}

impl RunEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunEventType::RunQueued => "run.queued",
            RunEventType::RunStarted => "run.started",
            RunEventType::SpecialistStarted => "specialist.started",
            RunEventType::SpecialistProgress => "specialist.progress",
            RunEventType::SpecialistCompleted => "specialist.completed",
            RunEventType::SpecialistFailed => "specialist.failed",
            RunEventType::RunPartial => "run.partial",
            RunEventType::RunCheckpointed => "run.checkpointed",
            RunEventType::RunRetryScheduled => "run.retry_scheduled",
            RunEventType::RunCancellationRequested => "run.cancellation_requested",
            RunEventType::RunCancelled => "run.cancelled",
            RunEventType::RunCompleted => "run.completed",
            RunEventType::RunFailed => "run.failed",
            RunEventType::ApprovalRequested => "approval.requested",
            RunEventType::ApprovalRecorded => "approval.recorded",
            RunEventType::MutationQueued => "mutation.queued",
            RunEventType::MutationCompleted => "mutation.completed",
            RunEventType::MutationFailed => "mutation.failed",
        }
    }
}

/// Mirrors RunEvent.visibility (always `room_and_owner`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventVisibility {
    RoomAndOwner,
}

/// Mirrors RunEvent in packages/contracts/src/events.ts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEvent {
    pub id: String,
    pub run_id: String,
    pub sequence: u64,
    #[serde(rename = "type")]
    pub event_type: RunEventType,
    pub version: u32,
    pub occurred_at: String,
    pub visibility: EventVisibility,
    pub payload: serde_json::Value,
}
```

Append to `crates/api-client/src/sse.rs` (after `parse_sse_frame`; this uncomments the contract usage from Task 4.1):

```rust
/// Terminal event types. Once one of these is accepted, the run is over and
/// the mobile/TUI stops consuming further events (same policy as the mobile's
/// TERMINAL_TYPES set).
pub const TERMINAL_EVENT_TYPES: &[RunEventType] = &[
    RunEventType::RunCompleted,
    RunEventType::RunPartial,
    RunEventType::RunFailed,
    RunEventType::RunCancelled,
];

pub fn is_terminal_event(event: &RunEvent) -> bool {
    TERMINAL_EVENT_TYPES.contains(&event.event_type)
}

impl RunEvent {
    /// Validate a candidate parsed from an SSE frame. Returns None when the
    /// event is malformed, belongs to a different run, or the frame's
    /// `event:` name disagrees with the JSON `type` — the stream skips it.
    /// Mirrors `eventFromFrame` + the zod `RunEvent` schema.
    pub fn from_sse_frame(frame: &SseFrame, expected_run_id: &str) -> Option<RunEvent> {
        let id = frame.id.as_deref()?;
        let event_name = frame.event.as_deref()?;
        if !id.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let sequence: u64 = id.parse().ok()?;
        let data: serde_json::Value = serde_json::from_str(&frame.data).ok()?;
        let event = Self::validate(data, expected_run_id, sequence)?;
        if event.event_type.as_str() != event_name {
            return None;
        }
        Some(event)
    }

    /// Validate a candidate event object against the wire contract.
    pub fn validate(value: serde_json::Value, expected_run_id: &str, expected_sequence: u64) -> Option<RunEvent> {
        let event: RunEvent = serde_json::from_value(value).ok()?;
        if event.id.is_empty() || event.run_id.is_empty() {
            return None;
        }
        if event.run_id != expected_run_id {
            return None;
        }
        if event.sequence != expected_sequence {
            return None;
        }
        if !event.payload.is_object() {
            return None;
        }
        Some(event)
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p api-client`
Expected: `test result: ok. 43 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/contract.rs crates/api-client/src/sse.rs
git commit -m "feat(api-client): RunEvent validation and event type enum"
```

### Task 4.3: `RunEventBuffer` — ordered accept + terminal dedupe policy

**Files:**
- Modify: `crates/api-client/src/sse.rs`

- [ ] **Step 1: Write the failing tests (append to the `tests` module in `crates/api-client/src/sse.rs`)**

```rust
fn sample_event(sequence: u64, event_type: RunEventType) -> RunEvent {
    RunEvent {
        id: format!("ev_{sequence}"),
        run_id: "r1".to_string(),
        sequence,
        event_type,
        version: 1,
        occurred_at: "2026-08-15T00:00:00.000Z".to_string(),
        visibility: EventVisibility::RoomAndOwner,
        payload: serde_json::json!({}),
    }
}

#[test]
fn buffer_accepts_strictly_increasing_sequences() {
    let mut buffer = RunEventBuffer::new();
    assert!(buffer.accept(sample_event(1, RunEventType::RunQueued)));
    assert!(buffer.accept(sample_event(2, RunEventType::RunStarted)));
    assert_eq!(buffer.highest_sequence(), 2);
    assert_eq!(buffer.events().len(), 2);
    assert!(!buffer.is_terminal());
}

#[test]
fn buffer_rejects_duplicate_and_out_of_order_events() {
    let mut buffer = RunEventBuffer::new();
    assert!(buffer.accept(sample_event(2, RunEventType::RunStarted)));
    assert!(!buffer.accept(sample_event(2, RunEventType::RunStarted)), "duplicate rejected");
    assert!(!buffer.accept(sample_event(1, RunEventType::RunQueued)), "stale rejected");
    assert_eq!(buffer.events().len(), 1);
}

#[test]
fn terminal_event_stops_further_acceptance() {
    let mut buffer = RunEventBuffer::new();
    assert!(buffer.accept(sample_event(1, RunEventType::RunStarted)));
    assert!(buffer.accept(sample_event(2, RunEventType::RunCompleted)));
    assert!(buffer.is_terminal());
    assert!(!buffer.accept(sample_event(3, RunEventType::RunStarted)), "post-terminal ignored");
    assert!(!buffer.accept(sample_event(3, RunEventType::RunCompleted)), "stale terminal ignored");
    assert_eq!(buffer.events().len(), 2);
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client buffer_accepts_strictly_increasing_sequences`
Expected: compile error — `cannot find type \`RunEventBuffer\``.

- [ ] **Step 3: Write the implementation (append to `crates/api-client/src/sse.rs`)**

```rust
/// Ordered buffer of accepted run events. Once a terminal event is accepted,
/// later events are ignored — stale terminal events and duplicates never
/// re-enter the timeline (same policy as the mobile's run store).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunEventBuffer {
    events: Vec<RunEvent>,
    terminal: bool,
}

impl RunEventBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> &[RunEvent] {
        &self.events
    }

    pub fn is_terminal(&self) -> bool {
        self.terminal
    }

    pub fn highest_sequence(&self) -> u64 {
        self.events.last().map(|event| event.sequence).unwrap_or(0)
    }

    /// Try to accept an event. Returns true when accepted; false when the
    /// buffer is already terminal or the sequence is not strictly greater
    /// than the last accepted one.
    pub fn accept(&mut self, event: RunEvent) -> bool {
        if self.terminal {
            return false;
        }
        if event.sequence <= self.highest_sequence() {
            return false;
        }
        if is_terminal_event(&event) {
            self.terminal = true;
        }
        self.events.push(event);
        true
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p api-client`
Expected: `test result: ok. 46 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/sse.rs
git commit -m "feat(api-client): run event buffer with terminal dedupe"
```

### Task 4.4: `EventStream` — happy path against the mock server

**Files:**
- Modify: `crates/api-client/src/sse.rs`

- [ ] **Step 1: Write the failing test (append to the `tests` module in `crates/api-client/src/sse.rs`)**

```rust
fn event_frame(sequence: u64, event_type: &str) -> String {
    let json = serde_json::json!({
        "id": format!("ev_{sequence}"),
        "runId": "r1",
        "sequence": sequence,
        "type": event_type,
        "version": 1,
        "occurredAt": "2026-08-15T00:00:00.000Z",
        "visibility": "room_and_owner",
        "payload": { "note": "x" }
    });
    format!("id: {sequence}\nevent: {event_type}\ndata: {json}\n\n")
}

#[tokio::test]
async fn stream_yields_events_in_order_then_ends_at_terminal() {
    let server = MockServer::start_async().await;
    let body = event_frame(1, "run.queued")
        + &event_frame(2, "specialist.started")
        + &event_frame(3, "run.completed");
    let mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/api/runs/r1/events")
                .query_param("after", "0")
                .header("cookie", "cp_session=abc123");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(body);
        })
        .await;

    let mut stream = EventStream::new(&server.base_url(), "cp_session=abc123", "r1", 0);
    let mut sequences = Vec::new();
    for _ in 0..8 {
        match stream.next().await {
            Some(Ok(StreamEvent::Run(event))) => {
                sequences.push(event.sequence);
                if stream.is_terminal() {
                    break;
                }
            }
            Some(Ok(StreamEvent::Reconnecting { .. })) => panic!("no reconnect expected"),
            Some(Err(error)) => panic!("unexpected error: {error}"),
            None => break,
        }
    }
    assert_eq!(sequences, vec![1, 2, 3]);
    assert!(stream.is_terminal());
    assert!(stream.next().await.is_none());
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client stream_yields_events_in_order_then_ends_at_terminal`
Expected: compile error — `cannot find type \`EventStream\``.

- [ ] **Step 3: Write the implementation (append to `crates/api-client/src/sse.rs`)**

```rust
/// What the stream yields: a validated run event, or a notification that the
/// connection dropped and the stream is reconnecting from `after`.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    Run(RunEvent),
    Reconnecting { attempt: u32, after: u64 },
}

/// A resumable SSE stream over GET /api/runs/:runId/events. Yields validated
/// `RunEvent`s; on a dropped connection it reconnects with `?after=<last
/// sequence>` and exponential backoff. Ends (returns None) after a terminal
/// event or a fatal error is returned via `Err`.
pub struct EventStream {
    http: reqwest::Client,
    base_url: String,
    cookie: String,
    run_id: String,
    after: u64,
    terminal: bool,
    reconnect_attempt: u32,
    next_reconnect: Option<Instant>,
    base_delay_ms: u64,
    max_delay_ms: u64,
    pending: VecDeque<RunEvent>,
    buffer: String,
}

impl EventStream {
    pub fn new(base_url: &str, cookie: &str, run_id: &str, after: u64) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.trim().trim_end_matches('/').to_string(),
            cookie: cookie.to_string(),
            run_id: run_id.to_string(),
            after,
            terminal: false,
            reconnect_attempt: 0,
            next_reconnect: None,
            base_delay_ms: 500,
            max_delay_ms: 8_000,
            pending: VecDeque::new(),
            buffer: String::new(),
        }
    }

    /// Override reconnect delays (tests use short values).
    pub fn with_reconnect_delays(mut self, base_delay_ms: u64, max_delay_ms: u64) -> Self {
        self.base_delay_ms = base_delay_ms.max(10);
        self.max_delay_ms = self.base_delay_ms.max(max_delay_ms);
        self
    }

    pub fn is_terminal(&self) -> bool {
        self.terminal
    }

    fn reconnect_delay(&self) -> Duration {
        let exponential = (self.base_delay_ms as f64 * 2f64.powi(self.reconnect_attempt as i32))
            .min(self.max_delay_ms as f64);
        Duration::from_millis(exponential.max(10.0) as u64)
    }

    fn schedule_reconnect(&mut self) {
        if self.next_reconnect.is_none() {
            self.next_reconnect = Some(Instant::now() + self.reconnect_delay());
        }
    }

    /// Read one chunk of the body and validate any complete frames.
    fn consume(&mut self, chunk: &[u8]) -> Result<(), ControlPlaneError> {
        self.buffer.push_str(&String::from_utf8_lossy(chunk).replace("\r\n", "\n"));
        while let Some(boundary) = self.buffer.find("\n\n") {
            let frame_text = self.buffer[..boundary].to_string();
            self.buffer.drain(..boundary + 2);
            if self.terminal {
                continue;
            }
            if let Some(frame) = parse_sse_frame(&frame_text) {
                if let Some(event) = RunEvent::from_sse_frame(&frame, &self.run_id) {
                    if event.sequence > self.after {
                        self.pending.push_back(event);
                    }
                }
            }
            if self.terminal {
                break;
            }
        }
        Ok(())
    }

    async fn open_and_read(&mut self) -> Result<(), ControlPlaneError> {
        let url = format!(
            "{}/api/runs/{}/events?after={}",
            self.base_url,
            urlencode(&self.run_id),
            self.after
        );
        let response = self
            .http
            .get(url)
            .header(reqwest::header::COOKIE, &self.cookie)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .send()
            .await
            .map_err(ControlPlaneError::Http)?;
        if response.status() == StatusCode::UNAUTHORIZED {
            return Err(ControlPlaneError::SessionExpired);
        }
        if response.status() == StatusCode::NOT_FOUND {
            return Err(ControlPlaneError::Api {
                status: 404,
                code: Some("RUN_NOT_FOUND".to_string()),
                message: "Run not found".to_string(),
            });
        }
        if response.status() == StatusCode::TOO_MANY_REQUESTS || response.status().is_server_error() {
            self.schedule_reconnect();
            return Ok(());
        }
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(ControlPlaneError::Http)?;
            self.consume(&chunk)?;
            if self.terminal {
                return Ok(());
            }
        }
        if self.terminal {
            return Ok(());
        }
        self.schedule_reconnect();
        Ok(())
    }

    /// Pull the next item from the stream.
    pub async fn next(&mut self) -> Option<Result<StreamEvent, ControlPlaneError>> {
        loop {
            if self.terminal {
                return None;
            }
            if let Some(event) = self.pending.pop_front() {
                self.after = event.sequence;
                if is_terminal_event(&event) {
                    self.terminal = true;
                }
                return Some(Ok(StreamEvent::Run(event)));
            }
            if let Some(wait) = self.next_reconnect.take() {
                tokio::time::sleep_until(wait.into()).await;
                self.reconnect_attempt += 1;
                return Some(Ok(StreamEvent::Reconnecting {
                    attempt: self.reconnect_attempt,
                    after: self.after,
                }));
            }
            match self.open_and_read().await {
                Ok(()) => {}
                Err(error) => return Some(Err(error)),
            }
        }
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p api-client`
Expected: `test result: ok. 47 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/sse.rs
git commit -m "feat(api-client): resumable SSE event stream"
```

### Task 4.5: `EventStream` — reconnect with resume-from-last-sequence, malformed frames ignored mid-stream

**Files:**
- Modify: `crates/api-client/src/sse.rs`

- [ ] **Step 1: Write the failing tests (append to the `tests` module in `crates/api-client/src/sse.rs`)**

```rust
#[tokio::test]
async fn stream_resumes_from_last_sequence_after_drop() {
    let server = MockServer::start_async().await;
    let first = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/api/runs/r1/events")
                .query_param("after", "0");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(event_frame(1, "run.queued") + &event_frame(2, "specialist.started"));
        })
        .await;
    let second = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/api/runs/r1/events")
                .query_param("after", "2");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(event_frame(3, "run.completed"));
        })
        .await;

    let mut stream = EventStream::new(&server.base_url(), "cp_session=abc123", "r1", 0)
        .with_reconnect_delays(10, 50);
    let mut sequences = Vec::new();
    let mut reconnect_afters = Vec::new();
    for _ in 0..12 {
        match stream.next().await {
            Some(Ok(StreamEvent::Run(event))) => {
                sequences.push(event.sequence);
                if stream.is_terminal() {
                    break;
                }
            }
            Some(Ok(StreamEvent::Reconnecting { after, .. })) => {
                reconnect_afters.push(after);
            }
            Some(Err(error)) => panic!("unexpected error: {error}"),
            None => break,
        }
    }
    assert_eq!(sequences, vec![1, 2, 3]);
    assert_eq!(reconnect_afters, vec![2], "resume cursor must be the last sequence");
    first.assert_async().await;
    second.assert_async().await;
}

#[tokio::test]
async fn stream_skips_malformed_frames_without_stopping() {
    let server = MockServer::start_async().await;
    let garbage = ": heartbeat\n\n"
        + "id: not-a-number\nevent: run.started\ndata: {}\n\n"
        + "id: 1\nevent: run.started\ndata: not json\n\n"
        + &event_frame(1, "run.queued")
        + "id: 2\nevent: run.started\ndata: {\"id\":\"ev_x\",\"runId\":\"OTHER_RUN\",\"sequence\":2,\"type\":\"run.started\",\"version\":1,\"occurredAt\":\"2026-08-15T00:00:00.000Z\",\"visibility\":\"room_and_owner\",\"payload\":{}}\n\n"
        + &event_frame(3, "run.completed");
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/api/runs/r1/events");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(garbage);
        })
        .await;

    let mut stream = EventStream::new(&server.base_url(), "cp_session=abc123", "r1", 0);
    let mut sequences = Vec::new();
    for _ in 0..8 {
        match stream.next().await {
            Some(Ok(StreamEvent::Run(event))) => {
                sequences.push(event.sequence);
                if stream.is_terminal() {
                    break;
                }
            }
            Some(Ok(StreamEvent::Reconnecting { .. })) => panic!("no reconnect expected"),
            Some(Err(error)) => panic!("unexpected error: {error}"),
            None => break,
        }
    }
    assert_eq!(sequences, vec![1, 3], "only valid events for this run are accepted");
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client stream_resumes_from_last_sequence_after_drop`
Expected: with Task 4.4 complete this already passes — the TDD red for this behavior existed implicitly when `EventStream::next` first connected without resuming; the assertions here (`reconnect_afters == vec![2]` and `sequences == vec![1, 2, 3]`) now lock the resume behavior. To observe a red run, temporarily change the second mock's `query_param("after", "2")` to `query_param("after", "99")` → `test result: FAILED` (second mock never matched, `reconnect_afters` empty) — then revert. Confirm the green run in Step 3.

- [ ] **Step 3: No new implementation needed beyond Task 4.4 — run the suite**

Run: `cargo test -p api-client`
Expected: `test result: ok. 49 passed; 0 failed; ...`

- [ ] **Step 4: Commit**

```bash
git add crates/api-client/src/sse.rs
git commit -m "test(api-client): stream resume and malformed-frame resilience"
```

### Task 4.6: `EventStream` — 401 and 404 mapping

**Files:**
- Modify: `crates/api-client/src/sse.rs`

- [ ] **Step 1: Write the failing tests (append to the `tests` module in `crates/api-client/src/sse.rs`)**

```rust
#[tokio::test]
async fn stream_maps_401_to_session_expired() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/api/runs/r1/events");
            then.status(401);
        })
        .await;

    let mut stream = EventStream::new(&server.base_url(), "cp_session=stale", "r1", 0);
    let error = stream.next().await.expect("yields an error").unwrap_err();
    assert!(error.is_session_expired());
}

#[tokio::test]
async fn stream_maps_404_to_run_not_found() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/api/runs/r1/events");
            then.status(404).json_body(serde_json::json!({
                "error": { "code": "RUN_NOT_FOUND", "message": "Run not found", "requestId": "req_1" }
            }));
        })
        .await;

    let mut stream = EventStream::new(&server.base_url(), "cp_session=abc123", "r1", 0);
    let error = stream.next().await.expect("yields an error").unwrap_err();
    assert_eq!(error.status(), Some(404));
    assert_eq!(error.code(), Some("RUN_NOT_FOUND"));
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client stream_maps_401_to_session_expired`
Expected: FAIL — with Task 4.4 complete this should already pass; if it does, confirm green in Step 3. (The TDD red for these two branches existed implicitly when `open_and_read` first ignored status codes; the tests now lock the behavior.)

- [ ] **Step 3: Run the suite**

Run: `cargo test -p api-client`
Expected: `test result: ok. 51 passed; 0 failed; ...`

- [ ] **Step 4: Commit**

```bash
git add crates/api-client/src/sse.rs
git commit -m "test(api-client): stream 401 and 404 error mapping"
```

### Task 4.7: Export the SSE module surface

**Files:**
- Modify: `crates/api-client/src/lib.rs`

- [ ] **Step 1: Write the failing test — append to `crates/api-client/src/lib.rs`**

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn sse_surface_is_public() {
        let _: fn() -> Option<crate::sse::SseFrame> = crate::sse::parse_sse_frame;
        let _ = crate::sse::is_terminal_event;
        let _: Option<crate::sse::RunEventBuffer> = None;
        let _: Option<crate::sse::EventStream> = None;
        let _ = crate::sse::StreamEvent::Reconnecting { attempt: 1, after: 0 };
        let _: Option<crate::sse::TERMINAL_EVENT_TYPES> = None;
    }
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p api-client sse_surface_is_public`
Expected: FAIL with `error[E0603]: module \`sse\` is private` (currently `pub mod sse;` was added in Task 4.1 — if it is already `pub`, this passes; the meaningful check is the re-export in Step 3).

- [ ] **Step 3: Make the surface explicit in `crates/api-client/src/lib.rs`**

```rust
pub mod sse;

pub use sse::{EventStream, RunEventBuffer, SseFrame, StreamEvent, TERMINAL_EVENT_TYPES, is_terminal_event};
```

- [ ] **Step 4: Run the full suite**

Run: `cargo test -p api-client`
Expected: `test result: ok. 52 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/api-client/src/lib.rs
git commit -m "refactor(api-client): export sse surface"
```

---

# Group 5: state — `SessionStore`

The session lives at `~/.config/matrix-workspace-tui/session.json` (via the `dirs` crate), mode `0600`. It stores the control-plane cookie plus the list of workspaces this client has created (the backend has no list-workspaces endpoint, so the client keeps its own). Corrupted files surface a typed error; `clear()` removes the file.

### Task 5.1: `SessionData`, `SessionStore::save`/`load` roundtrip

**Files:**
- Create: `crates/state/src/session_store.rs`
- Modify: `crates/state/src/lib.rs`

- [ ] **Step 1: Write the failing test (in `crates/state/src/session_store.rs`, `#[cfg(test)] mod tests`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use api_client::WorkspaceSelection;
    use tempfile::tempdir;

    fn workspace(id: &str) -> WorkspaceSelection {
        WorkspaceSelection {
            workspace_id: id.to_string(),
            name: "ws".to_string(),
            owner_id: "@u:example.org".to_string(),
            status: "active".to_string(),
            created_at: "2026-08-15T00:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn save_then_load_round_trips_cookie_and_workspaces() {
        let dir = tempdir().unwrap();
        let store = SessionStore::at_path(dir.path().join("session.json"));

        let mut data = SessionData::default();
        data.cookie = Some("cp_session=abc123".to_string());
        data.workspaces.push(workspace("ws_1"));
        data.workspaces.push(workspace("ws_2"));

        store.save(&data).unwrap();
        let loaded = store.load().unwrap();

        assert_eq!(loaded.cookie.as_deref(), Some("cp_session=abc123"));
        assert_eq!(loaded.workspaces.len(), 2);
        assert_eq!(loaded.workspaces[1].workspace_id, "ws_2");
    }

    #[test]
    fn missing_file_loads_as_default() {
        let dir = tempdir().unwrap();
        let store = SessionStore::at_path(dir.path().join("session.json"));
        let loaded = store.load().unwrap();
        assert_eq!(loaded, SessionData::default());
    }
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p state save_then_load_round_trips_cookie_and_workspaces`
Expected: compile error — `cannot find type \`SessionStore\``.

- [ ] **Step 3: Write the implementation — full `crates/state/src/session_store.rs`**

```rust
use api_client::WorkspaceSelection;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Everything persisted on disk for the TUI session.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionData {
    pub cookie: Option<String>,
    /// Workspaces this client created (the backend has no list endpoint).
    pub workspaces: Vec<WorkspaceSelection>,
}

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("could not determine the config directory")]
    NoConfigDir,
    #[error("session file {path} is corrupted: {source}")]
    Corrupted {
        path: String,
        source: serde_json::Error,
    },
    #[error("could not read/write {path}: {source}")]
    Io { path: String, source: io::Error },
    #[error("could not serialize session data: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub struct SessionStore {
    path: PathBuf,
}

impl SessionStore {
    /// ~/.config/matrix-workspace-tui/session.json (via the `dirs` crate).
    pub fn default_path() -> Result<PathBuf, StateError> {
        let dir = dirs::config_dir().ok_or(StateError::NoConfigDir)?;
        Ok(dir.join("matrix-workspace-tui").join("session.json"))
    }

    pub fn at_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<SessionData, StateError> {
        match fs::read(&self.path) {
            Ok(bytes) => {
                let data: SessionData = serde_json::from_slice(&bytes).map_err(|source| {
                    StateError::Corrupted {
                        path: self.path.display().to_string(),
                        source,
                    }
                })?;
                Ok(data)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(SessionData::default()),
            Err(source) => Err(StateError::Io {
                path: self.path.display().to_string(),
                source,
            }),
        }
    }

    /// Write the session file (creating parent dirs) and chmod it 0600.
    pub fn save(&self, data: &SessionData) -> Result<(), StateError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| StateError::Io {
                path: parent.display().to_string(),
                source,
            })?;
        }
        let json = serde_json::to_vec_pretty(data)?;
        fs::write(&self.path, &json).map_err(|source| StateError::Io {
            path: self.path.display().to_string(),
            source,
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600)).map_err(
                |source| StateError::Io {
                    path: self.path.display().to_string(),
                    source,
                },
            )?;
        }
        Ok(())
    }

    /// Remove the session file. Missing file is not an error.
    pub fn clear(&self) -> Result<(), StateError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(StateError::Io {
                path: self.path.display().to_string(),
                source,
            }),
        }
    }
}
```

- [ ] **Step 4: Wire the module into `crates/state/src/lib.rs`**

```rust
//! Session persistence and the screen state machine for matrix-workspace-tui.

pub mod session_store;
pub mod screens;

pub use session_store::{SessionData, SessionStore, StateError};
```

(`screens` is created in Group 6 — until then the build of `state` will fail on the missing module. To keep this task green, add only `pub mod session_store;` now; Task 6.1 adds the `screens` module line.)

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test -p state`
Expected: `test result: ok. 2 passed; 0 failed; ...`

- [ ] **Step 6: Commit**

```bash
git add crates/state/src/session_store.rs crates/state/src/lib.rs
git commit -m "feat(state): session store save/load with workspaces"
```

### Task 5.2: File permissions 0600 and parent dir creation

**Files:**
- Modify: `crates/state/src/session_store.rs`

- [ ] **Step 1: Write the failing tests (append to the `tests` module)**

```rust
#[test]
fn save_creates_parent_directories() {
    let dir = tempdir().unwrap();
    let nested = dir.path().join("a").join("b");
    let store = SessionStore::at_path(nested.join("session.json"));
    store.save(&SessionData::default()).unwrap();
    assert!(nested.join("session.json").exists());
}

#[cfg(unix)]
#[test]
fn save_sets_mode_0600() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempdir().unwrap();
    let store = SessionStore::at_path(dir.path().join("session.json"));
    store.save(&SessionData::default()).unwrap();
    let mode = fs::metadata(dir.path().join("session.json")).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600, "session file must not be world-readable");
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p state save_sets_mode_0600`
Expected: this should pass once `save` runs (Task 5.1 already sets 0600). To see the red: temporarily remove the `fs::set_permissions` block from `save`, run, observe `test result: FAILED` with the mode assertion, then restore it.

- [ ] **Step 3: Run the suite**

Run: `cargo test -p state`
Expected: `test result: ok. 4 passed; 0 failed; ...`

- [ ] **Step 4: Commit**

```bash
git add crates/state/src/session_store.rs
git commit -m "test(state): 0600 permissions and parent dir creation"
```

### Task 5.3: `clear` and corrupted-file handling

**Files:**
- Modify: `crates/state/src/session_store.rs`

- [ ] **Step 1: Write the failing tests (append to the `tests` module)**

```rust
#[test]
fn clear_removes_the_file_and_missing_file_is_not_an_error() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("session.json");
    let store = SessionStore::at_path(&path);
    store.save(&SessionData::default()).unwrap();
    store.clear().unwrap();
    assert!(!path.exists());
    store.clear().unwrap(); // second clear on a missing file is fine
}

#[test]
fn corrupted_file_surfaces_corrupted_error_then_clear_recovers() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("session.json");
    fs::write(&path, "{not valid json").unwrap();
    let store = SessionStore::at_path(&path);

    let error = store.load().unwrap_err();
    assert!(matches!(error, StateError::Corrupted { .. }));

    store.clear().unwrap();
    assert_eq!(store.load().unwrap(), SessionData::default());
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p state corrupted_file_surfaces_corrupted_error_then_clear_recovers`
Expected: `test result: FAILED` — `clear` exists but the corrupted load path was not yet exercised; with Task 5.1 complete this should already pass. If green, continue.

- [ ] **Step 3: Run the suite**

Run: `cargo test -p state`
Expected: `test result: ok. 6 passed; 0 failed; ...`

- [ ] **Step 4: Commit**

```bash
git add crates/state/src/session_store.rs
git commit -m "test(state): clear and corrupted file recovery"
```

### Task 5.4: `default_path` resolution

**Files:**
- Modify: `crates/state/src/session_store.rs`

- [ ] **Step 1: Write the failing test (append to the `tests` module)**

```rust
#[test]
fn default_path_points_into_config_dir() {
    let path = SessionStore::default_path().unwrap();
    let expected = dirs::config_dir().unwrap().join("matrix-workspace-tui").join("session.json");
    assert_eq!(path, expected);
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p state default_path_points_into_config_dir`
Expected: FAIL with `cannot find value \`dirs\`` — the `dirs` crate is not yet referenced in the test (add `use` via `dirs::config_dir()` fully qualified it is — the failure instead is that `default_path` does not exist yet if Task 5.1 was skipped; with Task 5.1 done it passes). Run once; expect `test result: ok`.

- [ ] **Step 3: Run the suite**

Run: `cargo test -p state`
Expected: `test result: ok. 7 passed; 0 failed; ...`

- [ ] **Step 4: Commit**

```bash
git add crates/state/src/session_store.rs
git commit -m "test(state): default config path"
```

---


---

# Group 6: state — screen state machine

All pure logic, no I/O. Each task adds one self-contained piece to `crates/state/src/screens.rs`; the `Screen` enum that ties the states together lands last (Task 6.8) so every task in this group is green on its own. `crates/state/src/lib.rs` already declares `pub mod screens;` from Task 5.1 — wait, it does not: Task 5.1 only added `pub mod session_store;`. This group adds the module line in Task 6.1.

### Task 6.1: `LoginState` + `WorkspacesState` (and wire the module)

**Files:**
- Create: `crates/state/src/screens.rs`
- Modify: `crates/state/src/lib.rs`

- [ ] **Step 1: Write the failing tests (in `crates/state/src/screens.rs`, `#[cfg(test)] mod tests`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use api_client::WorkspaceSelection;

    fn workspace(id: &str) -> WorkspaceSelection {
        WorkspaceSelection {
            workspace_id: id.to_string(),
            name: format!("ws {id}"),
            owner_id: "@u:example.org".to_string(),
            status: "active".to_string(),
            created_at: "2026-08-15T00:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn login_state_defaults_empty_and_validates() {
        let mut state = LoginState::default();
        assert_eq!(state.validation_error().as_deref(), Some("Homeserver URL is required"));
        state.set_homeserver_url("https://matrix.example.org".to_string());
        assert_eq!(state.validation_error().as_deref(), Some("Access token is required"));
        state.set_access_token("tok".to_string());
        assert!(state.validation_error().is_none());
    }

    #[test]
    fn login_state_rejects_non_http_urls() {
        let mut state = LoginState::default();
        state.set_homeserver_url("matrix.example.org".to_string());
        state.set_access_token("tok".to_string());
        assert_eq!(
            state.validation_error().as_deref(),
            Some("Homeserver URL must start with http:// or https://")
        );
    }

    #[test]
    fn login_state_editing_clears_previous_error() {
        let mut state = LoginState::default();
        state.error = Some("boom".to_string());
        state.set_homeserver_url("https://matrix.example.org".to_string());
        assert_eq!(state.error, None);
    }

    #[test]
    fn login_state_edits_the_focused_field() {
        let mut state = LoginState::default();
        state.insert_char('h');
        assert_eq!(state.homeserver_url, "h");
        state.toggle_focus();
        state.insert_text("tok_1");
        assert_eq!(state.access_token, "tok_1");
        state.backspace();
        assert_eq!(state.access_token, "tok_");
        state.toggle_focus();
        state.backspace();
        assert_eq!(state.homeserver_url, "");
    }

    #[test]
    fn workspaces_state_adds_and_selects() {
        let mut state = WorkspacesState::new();
        assert!(state.selected().is_none());
        state.add_workspace(workspace("ws_1"));
        state.add_workspace(workspace("ws_2"));
        assert_eq!(state.selected().unwrap().workspace_id, "ws_1");
        state.select_next();
        assert_eq!(state.selected().unwrap().workspace_id, "ws_2");
        state.select_next(); // clamps at the end
        assert_eq!(state.selected().unwrap().workspace_id, "ws_2");
        state.select_prev();
        assert_eq!(state.selected().unwrap().workspace_id, "ws_1");
        state.select_prev(); // clamps at the start
        assert_eq!(state.selected().unwrap().workspace_id, "ws_1");
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p state login_state_defaults_empty_and_validates`
Expected: compile error — `cannot find type \`LoginState\``.

- [ ] **Step 3: Write the implementation — top of `crates/state/src/screens.rs`**

```rust
use api_client::{
    AuditRecordItem, GithubMutationOperation, GithubPullRequestSummary, GithubRepositorySummary,
    GithubWriteGrantResult, GithubWriteScope, MatrixDelivery, RunEvent, RunMode, RunRequest,
    RoomSummary, WorkspaceSelection,
};
use api_client::sse::RunEventBuffer;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Specialist options offered by the composer (mirrors the mobile
/// navigator's `specialists` list).
pub const SPECIALISTS: &[(&str, &str)] = &[
    ("repo-reader", "Repository reader"),
    ("issue-reader", "Issue reader"),
    ("pr-reader", "Pull Request reader"),
];

/// Which login field receives typed characters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LoginField {
    #[default]
    HomeserverUrl,
    AccessToken,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LoginState {
    pub homeserver_url: String,
    pub access_token: String,
    pub focus: LoginField,
    pub error: Option<String>,
    pub submitting: bool,
}

impl LoginState {
    pub fn set_homeserver_url(&mut self, value: String) {
        self.homeserver_url = value;
        self.error = None;
    }

    pub fn set_access_token(&mut self, value: String) {
        self.access_token = value;
        self.error = None;
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            LoginField::HomeserverUrl => LoginField::AccessToken,
            LoginField::AccessToken => LoginField::HomeserverUrl,
        };
    }

    /// Insert one character into the focused field.
    pub fn insert_char(&mut self, c: char) {
        let target = match self.focus {
            LoginField::HomeserverUrl => &mut self.homeserver_url,
            LoginField::AccessToken => &mut self.access_token,
        };
        target.push(c);
        self.error = None;
    }

    /// Append a whole string (bracketed paste) into the focused field.
    pub fn insert_text(&mut self, text: &str) {
        let target = match self.focus {
            LoginField::HomeserverUrl => &mut self.homeserver_url,
            LoginField::AccessToken => &mut self.access_token,
        };
        target.push_str(text);
        self.error = None;
    }

    pub fn backspace(&mut self) {
        let target = match self.focus {
            LoginField::HomeserverUrl => &mut self.homeserver_url,
            LoginField::AccessToken => &mut self.access_token,
        };
        target.pop();
        self.error = None;
    }

    pub fn validation_error(&self) -> Option<String> {
        let url = self.homeserver_url.trim();
        if url.is_empty() {
            return Some("Homeserver URL is required".to_string());
        }
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Some("Homeserver URL must start with http:// or https://".to_string());
        }
        if self.access_token.trim().is_empty() {
            return Some("Access token is required".to_string());
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspacesState {
    pub workspaces: Vec<WorkspaceSelection>,
    pub selected: usize,
    pub error: Option<String>,
    pub creating: bool,
    pub name_input: String,
}

impl WorkspacesState {
    pub fn new() -> Self {
        Self {
            workspaces: Vec::new(),
            selected: 0,
            error: None,
            creating: false,
            name_input: String::new(),
        }
    }

    pub fn add_workspace(&mut self, workspace: WorkspaceSelection) {
        self.workspaces.push(workspace);
    }

    pub fn selected(&self) -> Option<&WorkspaceSelection> {
        self.workspaces.get(self.selected)
    }

    pub fn select_next(&mut self) {
        if !self.workspaces.is_empty() && self.selected + 1 < self.workspaces.len() {
            self.selected += 1;
        }
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn set_name_input(&mut self, value: String) {
        self.name_input = value;
        self.error = None;
    }
}
```

(Keep the `#[cfg(test)] mod tests` from Step 1 below it.)

- [ ] **Step 4: Wire the module into `crates/state/src/lib.rs`**

```rust
pub mod screens;
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test -p state`
Expected: `test result: ok. 12 passed; 0 failed; ...`

- [ ] **Step 6: Commit**

```bash
git add crates/state/src/screens.rs crates/state/src/lib.rs
git commit -m "feat(state): login and workspaces screen states"
```

### Task 6.2: `RoomsState` + `RoomBindingState`

**Files:**
- Modify: `crates/state/src/screens.rs`

- [ ] **Step 1: Write the failing tests (append to the `tests` module)**

```rust
fn room(id: &str, workspace_id: Option<&str>) -> RoomSummary {
    RoomSummary {
        room_id: id.to_string(),
        homeserver_url: "https://example.org".to_string(),
        display_name: Some(id.to_string()),
        workspace_id: workspace_id.map(|value| value.to_string()),
    }
}

#[test]
fn rooms_state_tracks_selection_and_binding() {
    let mut state = RoomsState::new("ws_1".to_string());
    assert!(state.selected_room().is_none());
    state.set_rooms(vec![room("!a:example.org", Some("ws_1")), room("!b:example.org", None)]);
    assert!(state.room_is_bound_to_workspace(), "first room is bound to ws_1");
    state.select_next();
    assert!(!state.room_is_bound_to_workspace(), "second room is unbound");
    assert_eq!(state.selected_room().unwrap().room_id, "!b:example.org");
}

#[test]
fn rooms_state_clamps_selection_when_list_shrinks() {
    let mut state = RoomsState::new("ws_1".to_string());
    state.set_rooms(vec![room("!a:example.org", None), room("!b:example.org", None)]);
    state.select_next();
    state.set_rooms(vec![room("!a:example.org", None)]);
    assert_eq!(state.selected(), 0);
    assert_eq!(state.selected_room().unwrap().room_id, "!a:example.org");
}

#[test]
fn rooms_state_marks_binding_after_bind() {
    let mut state = RoomsState::new("ws_1".to_string());
    state.set_rooms(vec![room("!a:example.org", None)]);
    assert!(!state.room_is_bound_to_workspace());
    state.mark_room_bound("!a:example.org");
    assert!(state.room_is_bound_to_workspace());
}

#[test]
fn room_binding_state_starts_pending_and_marks_bound() {
    let mut state = RoomBindingState::new(room("!a:example.org", None), "ws_1".to_string());
    assert!(!state.done);
    assert_eq!(state.room.room_id, "!a:example.org");
    assert_eq!(state.workspace_id, "ws_1");
    state.mark_bound();
    assert!(state.done);
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p state rooms_state_tracks_selection_and_binding`
Expected: compile error — `cannot find type \`RoomsState\``.

- [ ] **Step 3: Write the implementation (append to `crates/state/src/screens.rs`)**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct RoomsState {
    pub rooms: Vec<RoomSummary>,
    pub workspace_id: String,
    pub selected: usize,
    pub error: Option<String>,
    pub loading: bool,
}

impl RoomsState {
    pub fn new(workspace_id: String) -> Self {
        Self {
            rooms: Vec::new(),
            workspace_id,
            selected: 0,
            error: None,
            loading: false,
        }
    }

    pub fn set_rooms(&mut self, rooms: Vec<RoomSummary>) {
        self.rooms = rooms;
        if !self.rooms.is_empty() && self.selected >= self.rooms.len() {
            self.selected = self.rooms.len() - 1;
        }
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn selected_room(&self) -> Option<&RoomSummary> {
        self.rooms.get(self.selected)
    }

    pub fn select_next(&mut self) {
        if !self.rooms.is_empty() && self.selected + 1 < self.rooms.len() {
            self.selected += 1;
        }
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// True when the selected room is bound to this screen's workspace.
    pub fn room_is_bound_to_workspace(&self) -> bool {
        matches!(
            self.selected_room(),
            Some(room) if room.workspace_id.as_deref() == Some(self.workspace_id.as_str())
        )
    }

    /// Reflect a successful bind (POST binding) without refetching.
    pub fn mark_room_bound(&mut self, room_id: &str) {
        for room in &mut self.rooms {
            if room.room_id == room_id {
                room.workspace_id = Some(self.workspace_id.clone());
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RoomBindingState {
    pub room: RoomSummary,
    pub workspace_id: String,
    pub error: Option<String>,
    pub binding: bool,
    /// Set when the bind succeeded; the TUI pops back to Rooms on seeing it.
    pub done: bool,
}

impl RoomBindingState {
    pub fn new(room: RoomSummary, workspace_id: String) -> Self {
        Self {
            room,
            workspace_id,
            error: None,
            binding: false,
            done: false,
        }
    }

    pub fn mark_bound(&mut self) {
        self.done = true;
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p state`
Expected: `test result: ok. 16 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/state/src/screens.rs
git commit -m "feat(state): rooms and room binding screen states"
```

### Task 6.3: `RunComposerState`

**Files:**
- Modify: `crates/state/src/screens.rs`

- [ ] **Step 1: Write the failing tests (append to the `tests` module)**

```rust
#[test]
fn composer_requires_prompt_mode_and_specialists() {
    let mut state = RunComposerState::new("!a:example.org".to_string(), "ws_1".to_string());
    assert_eq!(state.validation_error().as_deref(), Some("Prompt is required"));
    state.set_prompt("Do the thing".to_string());
    assert_eq!(state.validation_error().as_deref(), Some("Choose a mode (parallel or sequential)"));
    state.toggle_mode(RunMode::Parallel);
    assert_eq!(state.validation_error().as_deref(), Some("Select at least one specialist"));
    state.toggle_specialist("pr-reader");
    assert!(state.validation_error().is_none());
}

#[test]
fn composer_toggles_specialists_and_mode() {
    let mut state = RunComposerState::new("!a:example.org".to_string(), "ws_1".to_string());
    state.toggle_specialist("repo-reader");
    state.toggle_specialist("pr-reader");
    assert_eq!(state.selected_specialists, vec!["repo-reader", "pr-reader"]);
    state.toggle_specialist("repo-reader"); // toggle off
    assert_eq!(state.selected_specialists, vec!["pr-reader"]);
    state.toggle_mode(RunMode::Sequential);
    assert_eq!(state.mode, Some(RunMode::Sequential));
}

#[test]
fn composer_request_requires_valid_input_and_carries_room_id() {
    let mut state = RunComposerState::new("!a:example.org".to_string(), "ws_1".to_string());
    assert!(state.request().is_none());
    state.set_prompt("  Do the thing  ".to_string());
    state.toggle_mode(RunMode::Parallel);
    state.toggle_specialist("repo-reader");
    let request = state.request().unwrap();
    assert_eq!(request.prompt, "Do the thing");
    assert_eq!(request.mode, RunMode::Parallel);
    assert_eq!(request.specialist_ids, vec!["repo-reader"]);
    assert_eq!(request.room_id.as_deref(), Some("!a:example.org"));
    assert_eq!(request.github_context, None);
    assert_eq!(state.workspace_id, "ws_1");
}

#[test]
fn composer_moves_the_specialist_cursor_and_toggles_at_cursor() {
    let mut state = RunComposerState::new("!a:example.org".to_string(), "ws_1".to_string());
    assert_eq!(state.specialist_cursor, 0);
    state.move_specialist_cursor_next();
    state.move_specialist_cursor_next();
    assert_eq!(state.specialist_cursor, 2);
    state.move_specialist_cursor_next(); // clamps
    assert_eq!(state.specialist_cursor, 2);
    state.toggle_specialist_at_cursor();
    assert_eq!(state.selected_specialists, vec!["pr-reader"]);
    state.move_specialist_cursor_prev();
    assert_eq!(state.specialist_cursor, 1);
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p state composer_requires_prompt_mode_and_specialists`
Expected: compile error — `cannot find type \`RunComposerState\``.

- [ ] **Step 3: Write the implementation (append to `crates/state/src/screens.rs`)**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct RunComposerState {
    pub prompt: String,
    pub mode: Option<RunMode>,
    pub selected_specialists: Vec<String>,
    pub room_id: String,
    pub workspace_id: String,
    /// Index into `SPECIALISTS` for space-to-toggle selection.
    pub specialist_cursor: usize,
    pub error: Option<String>,
    pub launching: bool,
}

impl RunComposerState {
    pub fn new(room_id: String, workspace_id: String) -> Self {
        Self {
            prompt: String::new(),
            mode: None,
            selected_specialists: Vec::new(),
            room_id,
            workspace_id,
            specialist_cursor: 0,
            error: None,
            launching: false,
        }
    }

    pub fn set_prompt(&mut self, prompt: String) {
        self.prompt = prompt;
        self.error = None;
    }

    pub fn toggle_mode(&mut self, mode: RunMode) {
        self.mode = Some(mode);
    }

    pub fn toggle_specialist(&mut self, id: &str) {
        if let Some(position) = self.selected_specialists.iter().position(|value| value == id) {
            self.selected_specialists.remove(position);
        } else {
            self.selected_specialists.push(id.to_string());
        }
    }

    pub fn move_specialist_cursor_next(&mut self) {
        if self.specialist_cursor + 1 < SPECIALISTS.len() {
            self.specialist_cursor += 1;
        }
    }

    pub fn move_specialist_cursor_prev(&mut self) {
        self.specialist_cursor = self.specialist_cursor.saturating_sub(1);
    }

    pub fn toggle_specialist_at_cursor(&mut self) {
        if let Some((id, _)) = SPECIALISTS.get(self.specialist_cursor) {
            self.toggle_specialist(id);
        }
    }

    pub fn validation_error(&self) -> Option<String> {
        if self.prompt.trim().is_empty() {
            return Some("Prompt is required".to_string());
        }
        if self.mode.is_none() {
            return Some("Choose a mode (parallel or sequential)".to_string());
        }
        if self.selected_specialists.is_empty() {
            return Some("Select at least one specialist".to_string());
        }
        None
    }

    /// The validated launch request, or None when the form is invalid.
    pub fn request(&self) -> Option<RunRequest> {
        if self.validation_error().is_some() {
            return None;
        }
        Some(RunRequest {
            prompt: self.prompt.trim().to_string(),
            mode: self.mode.unwrap(),
            specialist_ids: self.selected_specialists.clone(),
            room_id: Some(self.room_id.clone()),
            github_context: None,
        })
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p state`
Expected: `test result: ok. 19 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/state/src/screens.rs
git commit -m "feat(state): run composer state"
```

### Task 6.4: `RunState` (wraps the event buffer)

**Files:**
- Modify: `crates/state/src/screens.rs`

- [ ] **Step 1: Write the failing tests (append to the `tests` module)**

```rust
fn event(sequence: u64, event_type: api_client::RunEventType) -> RunEvent {
    RunEvent {
        id: format!("ev_{sequence}"),
        run_id: "r1".to_string(),
        sequence,
        event_type,
        version: 1,
        occurred_at: "2026-08-15T00:00:00.000Z".to_string(),
        visibility: api_client::EventVisibility::RoomAndOwner,
        payload: serde_json::json!({}),
    }
}

#[test]
fn run_state_accepts_events_and_detects_terminal() {
    let mut state = RunState::new("r1".to_string(), "ws_1".to_string());
    assert_eq!(state.highest_sequence(), 0);
    assert!(state.accept(event(1, api_client::RunEventType::RunStarted)));
    assert!(!state.is_terminal());
    assert!(state.accept(event(2, api_client::RunEventType::RunCompleted)));
    assert!(state.is_terminal());
    assert!(!state.accept(event(3, api_client::RunEventType::RunStarted)), "post-terminal rejected");
    assert_eq!(state.events().len(), 2);
}

#[test]
fn run_state_tracks_deliveries_cancel_and_reconnect() {
    let mut state = RunState::new("r1".to_string(), "ws_1".to_string());
    state.set_reconnecting(true);
    assert!(state.reconnecting);
    state.set_deliveries(vec![
        MatrixDelivery { sequence: 1, status: api_client::MatrixDeliveryStatus::Delivered },
        MatrixDelivery { sequence: 2, status: api_client::MatrixDeliveryStatus::Pending },
    ]);
    assert_eq!(state.deliveries.len(), 2);
    assert_eq!(state.deliveries[1].status, api_client::MatrixDeliveryStatus::Pending);
    state.request_cancel();
    assert!(state.cancel_requested);
    assert_eq!(state.error, None);
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p state run_state_accepts_events_and_detects_terminal`
Expected: compile error — `cannot find type \`RunState\``.

- [ ] **Step 3: Write the implementation (append to `crates/state/src/screens.rs`)**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct RunState {
    pub run_id: String,
    pub workspace_id: String,
    pub buffer: RunEventBuffer,
    pub deliveries: Vec<MatrixDelivery>,
    pub cancel_requested: bool,
    pub reconnecting: bool,
    pub error: Option<String>,
}

impl RunState {
    pub fn new(run_id: String, workspace_id: String) -> Self {
        Self {
            run_id,
            workspace_id,
            buffer: RunEventBuffer::new(),
            deliveries: Vec::new(),
            cancel_requested: false,
            reconnecting: false,
            error: None,
        }
    }

    pub fn accept(&mut self, event: RunEvent) -> bool {
        self.buffer.accept(event)
    }

    pub fn is_terminal(&self) -> bool {
        self.buffer.is_terminal()
    }

    pub fn events(&self) -> &[RunEvent] {
        self.buffer.events()
    }

    pub fn highest_sequence(&self) -> u64 {
        self.buffer.highest_sequence()
    }

    pub fn set_reconnecting(&mut self, value: bool) {
        self.reconnecting = value;
    }

    pub fn set_deliveries(&mut self, deliveries: Vec<MatrixDelivery>) {
        self.deliveries = deliveries;
    }

    pub fn request_cancel(&mut self) {
        self.cancel_requested = true;
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p state`
Expected: `test result: ok. 21 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/state/src/screens.rs
git commit -m "feat(state): run screen state with event buffer"
```

### Task 6.5: `GitHubWorkspaceState` + `GithubPanel` + `MutationConfirmationDraft`

**Files:**
- Modify: `crates/state/Cargo.toml` (add uuid)
- Modify: `crates/state/src/screens.rs`

- [ ] **Step 1: Add the uuid dependency to `crates/state/Cargo.toml`**

```toml
uuid.workspace = true
```

(add it to the `[dependencies]` block)

- [ ] **Step 2: Write the failing tests (append to the `tests` module in `crates/state/src/screens.rs`)**

```rust
#[test]
fn github_state_starts_on_repositories_panel() {
    let state = GitHubWorkspaceState::new("ws_1".to_string(), "r1".to_string(), Some("inst_9".to_string()));
    assert_eq!(state.panel, GithubPanel::Repositories);
    assert_eq!(state.installation_id.as_deref(), Some("inst_9"));
}

#[test]
fn github_state_switches_panels_and_clamps_selection() {
    let mut state = GitHubWorkspaceState::new("ws_1".to_string(), "r1".to_string(), None);
    state.set_repositories(vec![GithubRepositorySummary {
        id: 1,
        name: "repo".to_string(),
        full_name: "octo/repo".to_string(),
        owner: "octo".to_string(),
        private: false,
        default_branch: "main".to_string(),
        description: None,
        html_url: "https://github.com/octo/repo".to_string(),
        archived: false,
    }]);
    assert_eq!(state.selected_repository().as_deref(), Some("octo/repo"));
    state.switch_panel(GithubPanel::Audit);
    state.select_next(); // clamps to empty list
    assert_eq!(state.selected_index, 0);
}

#[test]
fn github_state_builds_mutation_confirmation_draft() {
    let mut state = GitHubWorkspaceState::new("ws_1".to_string(), "r1".to_string(), None);
    state.set_repositories(vec![GithubRepositorySummary {
        id: 1,
        name: "repo".to_string(),
        full_name: "octo/repo".to_string(),
        owner: "octo".to_string(),
        private: false,
        default_branch: "main".to_string(),
        description: None,
        html_url: "https://github.com/octo/repo".to_string(),
        archived: false,
    }]);
    let draft = state.begin_mutation("Test issue".to_string()).expect("draft");
    assert_eq!(draft.operation, GithubMutationOperation::CreateIssue);
    assert_eq!(draft.repository, "octo/repo");
    assert_eq!(draft.scope, GithubWriteScope::IssuesWrite);
    assert_eq!(draft.arguments["title"], "Test issue");
    assert!(!draft.idempotency_key.is_empty());
    assert_eq!(
        draft.command_hash,
        "22a9632d51b690e300e3ef7fb397048392bc84a388c4ef68beb0d42202815fd8",
        "must match the mobile/server canonical hash for this command"
    );
}

#[test]
fn github_state_begin_mutation_requires_repository_and_title() {
    let mut state = GitHubWorkspaceState::new("ws_1".to_string(), "r1".to_string(), None);
    assert!(state.begin_mutation("Test issue".to_string()).is_none(), "no repository selected");
    state.set_repositories(vec![GithubRepositorySummary {
        id: 1,
        name: "repo".to_string(),
        full_name: "octo/repo".to_string(),
        owner: "octo".to_string(),
        private: false,
        default_branch: "main".to_string(),
        description: None,
        html_url: "https://github.com/octo/repo".to_string(),
        archived: false,
    }]);
    assert!(state.begin_mutation("   ".to_string()).is_none(), "empty title rejected");
}
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cargo test -p state github_state_starts_on_repositories_panel`
Expected: compile error — `cannot find type \`GitHubWorkspaceState\``.

- [ ] **Step 4: Write the implementation (append to `crates/state/src/screens.rs`)**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GithubPanel {
    Repositories,
    Issues,
    PullRequests,
    Audit,
}

/// The mutation flow status mirror (mobile MutationConfirmationStatus).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationFlowStatus {
    Idle,
    Submitting,
    Submitted,
    Succeeded,
    Denied,
    Expired,
    Failed,
    Duplicate,
}

/// Everything shown on the explicit confirmation screen before enqueue.
#[derive(Debug, Clone, PartialEq)]
pub struct MutationConfirmationDraft {
    pub operation: GithubMutationOperation,
    pub repository: String,
    pub arguments: serde_json::Value,
    pub scope: GithubWriteScope,
    pub idempotency_key: String,
    pub command_hash: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GitHubWorkspaceState {
    pub workspace_id: String,
    pub run_id: String,
    pub installation_id: Option<String>,
    pub panel: GithubPanel,
    pub repositories: Vec<GithubRepositorySummary>,
    pub issues: Vec<api_client::GithubIssueSummary>,
    pub pulls: Vec<GithubPullRequestSummary>,
    pub audit: Vec<AuditRecordItem>,
    pub selected_index: usize,
    pub error: Option<String>,
    pub loading: bool,
    pub grant: Option<GithubWriteGrantResult>,
    pub mutation_title: String,
    pub mutation_mode: bool,
    pub confirmation: Option<MutationConfirmationDraft>,
    pub mutation_status: MutationFlowStatus,
    pub command_id: Option<String>,
}

impl GitHubWorkspaceState {
    pub fn new(workspace_id: String, run_id: String, installation_id: Option<String>) -> Self {
        Self {
            workspace_id,
            run_id,
            installation_id,
            panel: GithubPanel::Repositories,
            repositories: Vec::new(),
            issues: Vec::new(),
            pulls: Vec::new(),
            audit: Vec::new(),
            selected_index: 0,
            error: None,
            loading: false,
            grant: None,
            mutation_title: String::new(),
            mutation_mode: false,
            confirmation: None,
            mutation_status: MutationFlowStatus::Idle,
            command_id: None,
        }
    }

    pub fn switch_panel(&mut self, panel: GithubPanel) {
        self.panel = panel;
        self.selected_index = 0;
    }

    pub fn set_repositories(&mut self, repositories: Vec<GithubRepositorySummary>) {
        self.repositories = repositories;
        self.clamp_selection();
    }

    pub fn set_issues(&mut self, issues: Vec<api_client::GithubIssueSummary>) {
        self.issues = issues;
        self.clamp_selection();
    }

    pub fn set_pull_requests(&mut self, pulls: Vec<GithubPullRequestSummary>) {
        self.pulls = pulls;
        self.clamp_selection();
    }

    pub fn set_audit(&mut self, audit: Vec<AuditRecordItem>) {
        self.audit = audit;
        self.clamp_selection();
    }

    fn clamp_selection(&mut self) {
        let len = self.panel_items_len();
        if len > 0 && self.selected_index >= len {
            self.selected_index = len - 1;
        }
    }

    fn panel_items_len(&self) -> usize {
        match self.panel {
            GithubPanel::Repositories => self.repositories.len(),
            GithubPanel::Issues => self.issues.len(),
            GithubPanel::PullRequests => self.pulls.len(),
            GithubPanel::Audit => self.audit.len(),
        }
    }

    pub fn select_next(&mut self) {
        let len = self.panel_items_len();
        if len > 0 && self.selected_index + 1 < len {
            self.selected_index += 1;
        }
    }

    pub fn select_prev(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    /// The currently selected repository (its full `owner/repo` name).
    pub fn selected_repository(&self) -> Option<String> {
        self.repositories
            .get(self.selected_index)
            .map(|repo| repo.full_name.clone())
    }

    pub fn set_grant(&mut self, grant: GithubWriteGrantResult) {
        self.grant = Some(grant);
    }

    pub fn set_mutation_title(&mut self, title: String) {
        self.mutation_title = title;
        self.error = None;
    }

    pub fn set_mutation_status(&mut self, status: MutationFlowStatus) {
        self.mutation_status = status;
    }

    pub fn set_command_id(&mut self, command_id: Option<String>) {
        self.command_id = command_id;
    }

    /// Build the explicit confirmation draft. Requires a selected repository
    /// and a non-empty title. The operation is always `create_issue` with
    /// scope `issues:write` (the mobile's WRITE_SCOPE/OPERATION constants).
    pub fn begin_mutation(&mut self, title: String) -> Option<MutationConfirmationDraft> {
        let repository = self.selected_repository()?;
        if title.trim().is_empty() {
            return None;
        }
        let operation = GithubMutationOperation::CreateIssue;
        let scope = GithubWriteScope::IssuesWrite;
        let arguments = serde_json::json!({ "title": title.trim() });
        let idempotency_key = uuid::Uuid::new_v4().to_string();
        let command_hash = command_hash(operation, &arguments);
        Some(MutationConfirmationDraft {
            operation,
            repository,
            arguments,
            scope,
            idempotency_key,
            command_hash,
        })
    }
}
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test -p state github_state_starts_on_repositories_panel github_state_switches_panels_and_clamps_selection github_state_begin_mutation_requires_repository_and_title`
Expected: the three tests that do not touch `command_hash` pass; `github_state_builds_mutation_confirmation_draft` still fails to compile until Task 6.6 adds `command_hash`. Run the full suite in Step 4 of Task 6.6.

- [ ] **Step 6: Commit**

```bash
git add crates/state/Cargo.toml crates/state/src/screens.rs
git commit -m "feat(state): github workspace state with mutation draft"
```

### Task 6.6: Mutation helpers — `confirmation_sentence`, `canonicalize`, `command_hash`

**Files:**
- Modify: `crates/state/src/screens.rs`

- [ ] **Step 1: Write the failing tests (append to the `tests` module)**

```rust
#[test]
fn confirmation_sentence_matches_mobile_format() {
    assert_eq!(
        confirmation_sentence(
            GithubMutationOperation::CreateIssue,
            "octo/repo",
            GithubWriteScope::IssuesWrite,
        ),
        "I confirm create issue on octo/repo (issues:write)"
    );
    assert_eq!(
        confirmation_sentence(
            GithubMutationOperation::CreatePrComment,
            "octo/repo",
            GithubWriteScope::PullRequestsWrite,
        ),
        "I confirm comment on pull request on octo/repo (pull_requests:write)"
    );
}

#[test]
fn canonicalize_sorts_keys_recursively() {
    let value = serde_json::json!({
        "operation": "create_issue",
        "arguments": { "title": "x", "body": "y" },
    });
    let canonical = canonicalize(&value);
    assert_eq!(
        serde_json::to_string(&canonical).unwrap(),
        r#"{"arguments":{"body":"y","title":"x"},"operation":"create_issue"}"#
    );
}

#[test]
fn command_hash_matches_the_control_plane_vector() {
    let hash = command_hash(
        GithubMutationOperation::CreateIssue,
        &serde_json::json!({ "title": "Test issue" }),
    );
    assert_eq!(hash, "22a9632d51b690e300e3ef7fb397048392bc84a388c4ef68beb0d42202815fd8");

    let hash_with_body = command_hash(
        GithubMutationOperation::CreateIssue,
        &serde_json::json!({ "body": "Details", "title": "Test issue" }),
    );
    assert_eq!(hash_with_body, "8c8a0ab437a3a0c5760a8179ab81bcc9b84b31878cf2dede2888c63fa8b4d2b9");
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p state command_hash_matches_the_control_plane_vector`
Expected: compile error — `cannot find function \`command_hash\``.

- [ ] **Step 3: Write the implementation (append to `crates/state/src/screens.rs`)**

```rust
/// The exact confirmation sentence the mobile sends as `confirmationText`
/// (mirrors confirmationSentence in
/// apps/mobile/src/components/MutationConfirmation.tsx).
pub fn confirmation_sentence(
    operation: GithubMutationOperation,
    repository: &str,
    scope: GithubWriteScope,
) -> String {
    let label = match operation {
        GithubMutationOperation::CreateIssue => "create issue",
        GithubMutationOperation::UpdateIssue => "update issue",
        GithubMutationOperation::CommentIssue => "comment on issue",
        GithubMutationOperation::CreatePrComment => "comment on pull request",
    };
    let scope_name = match scope {
        GithubWriteScope::IssuesWrite => "issues:write",
        GithubWriteScope::PullRequestsWrite => "pull_requests:write",
    };
    format!("I confirm {label} on {repository} ({scope_name})")
}

/// Recursively sort object keys; must match the control-plane
/// canonicalization (mutation-command.ts `canonicalize`).
pub fn canonicalize(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(canonicalize).collect())
        }
        serde_json::Value::Object(map) => {
            let mut sorted = BTreeMap::new();
            for (key, value) in map {
                sorted.insert(key.clone(), canonicalize(value));
            }
            serde_json::Value::Object(sorted.into_iter().collect())
        }
        other => other.clone(),
    }
}

fn operation_name(operation: GithubMutationOperation) -> &'static str {
    match operation {
        GithubMutationOperation::CreateIssue => "create_issue",
        GithubMutationOperation::UpdateIssue => "update_issue",
        GithubMutationOperation::CommentIssue => "comment_issue",
        GithubMutationOperation::CreatePrComment => "create_pr_comment",
    }
}

/// SHA-256 of the canonical `{"operation": ..., "arguments": ...}` JSON —
/// byte-identical to the server's computeCommandHash.
pub fn command_hash(
    operation: GithubMutationOperation,
    arguments: &serde_json::Value,
) -> String {
    let value = canonicalize(&serde_json::json!({
        "operation": operation_name(operation),
        "arguments": arguments,
    }));
    let canonical = serde_json::to_string(&value).expect("command is serializable");
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    format!("{:x}", hasher.finalize())
}
```

- [ ] **Step 4: Run the full state suite**

Run: `cargo test -p state`
Expected: `test result: ok. 24 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/state/src/screens.rs
git commit -m "feat(state): mutation confirmation sentence and canonical command hash"
```

### Task 6.7: `ScreenId` + `Screen` enum

**Files:**
- Modify: `crates/state/src/screens.rs`

- [ ] **Step 1: Write the failing test (append to the `tests` module)**

```rust
#[test]
fn every_screen_reports_its_id() {
    assert_eq!(Screen::Login(LoginState::default()).id(), ScreenId::Login);
    assert_eq!(Screen::Workspaces(WorkspacesState::new()).id(), ScreenId::Workspaces);
    assert_eq!(Screen::Rooms(RoomsState::new("ws_1".to_string())).id(), ScreenId::Rooms);
    assert_eq!(
        Screen::RoomBinding(RoomBindingState::new(
            room("!a:example.org", None),
            "ws_1".to_string(),
        ))
        .id(),
        ScreenId::RoomBinding
    );
    assert_eq!(Screen::RunComposer(RunComposerState::new("!a:example.org".to_string(), "ws_1".to_string())).id(), ScreenId::RunComposer);
    assert_eq!(Screen::Run(RunState::new("r1".to_string(), "ws_1".to_string())).id(), ScreenId::Run);
    assert_eq!(
        Screen::GitHubWorkspace(GitHubWorkspaceState::new("ws_1".to_string(), "r1".to_string(), None)).id(),
        ScreenId::GitHubWorkspace
    );
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p state every_screen_reports_its_id`
Expected: compile error — `cannot find type \`Screen\``.

- [ ] **Step 3: Write the implementation (append to `crates/state/src/screens.rs`)**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenId {
    Login,
    Workspaces,
    Rooms,
    RoomBinding,
    RunComposer,
    Run,
    GitHubWorkspace,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Login(LoginState),
    Workspaces(WorkspacesState),
    Rooms(RoomsState),
    RoomBinding(RoomBindingState),
    RunComposer(RunComposerState),
    Run(RunState),
    GitHubWorkspace(GitHubWorkspaceState),
}

impl Screen {
    pub fn id(&self) -> ScreenId {
        match self {
            Screen::Login(_) => ScreenId::Login,
            Screen::Workspaces(_) => ScreenId::Workspaces,
            Screen::Rooms(_) => ScreenId::Rooms,
            Screen::RoomBinding(_) => ScreenId::RoomBinding,
            Screen::RunComposer(_) => ScreenId::RunComposer,
            Screen::Run(_) => ScreenId::Run,
            Screen::GitHubWorkspace(_) => ScreenId::GitHubWorkspace,
        }
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p state`
Expected: `test result: ok. 25 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/state/src/screens.rs
git commit -m "feat(state): screen enum with ids"
```

### Task 6.8: Full state suite gate

**Files:**
- none (verification only)

- [ ] **Step 1: Run the full workspace test suite**

Run: `cargo test --workspace`
Expected: `test result: ok.` for `api-client` (52 tests) and `state` (25 tests); the `tui` crate has no tests yet and its placeholder main compiles.

- [ ] **Step 2: Commit any drift (e.g. Cargo.lock)**

```bash
git add Cargo.lock
git commit -m "chore: lockfile after state additions" || echo "no changes to commit"
```

---

# Group 7: tui — app shell

The `tui` crate is the binary. This group builds the core: the `Command`/`AppEvent` types, the screen-stack router, the async command executor, the run-loop, and `main.rs`. Group 8 then fills in each screen's key handler + render (the files created here as stubs get **replaced entirely** there).

### Task 7.1: `App` skeleton — screen stack, session restore, error types

**Files:**
- Create: `crates/tui/src/app.rs`
- Modify: `crates/tui/src/main.rs` (keep placeholder for now)
- Create: `crates/tui/src/screens/mod.rs` + six stub modules (created in Task 7.2; this task only needs `screens/mod.rs` empty so `crate::screens` resolves — create the file with `// screen modules land in Group 8` and no items)

- [ ] **Step 1: Write the failing tests (in `crates/tui/src/app.rs`, `#[cfg(test)] mod tests`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use state::screens::{Screen, ScreenId};
    use state::session_store::SessionStore;
    use tempfile::tempdir;

    #[test]
    fn app_starts_on_login_without_session() {
        let dir = tempdir().unwrap();
        let app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
        assert_eq!(app.current().id(), ScreenId::Login);
        assert_eq!(app.stack.len(), 1);
    }

    #[test]
    fn app_starts_on_workspaces_with_stored_session() {
        let dir = tempdir().unwrap();
        let store = SessionStore::at_path(dir.path().join("session.json"));
        let mut data = SessionData::default();
        data.cookie = Some("cp_session=abc123".to_string());
        data.workspaces.push(WorkspaceSelection {
            workspace_id: "ws_1".to_string(),
            name: "My workspace".to_string(),
            owner_id: "@u:example.org".to_string(),
            status: "active".to_string(),
            created_at: "2026-08-15T00:00:00.000Z".to_string(),
        });
        store.save(&data).unwrap();

        let app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
        assert_eq!(app.current().id(), ScreenId::Workspaces);
        assert_eq!(app.client.cookie(), Some("cp_session=abc123"));
        let Screen::Workspaces(state) = app.current() else { panic!("workspaces") };
        assert_eq!(state.workspaces.len(), 1);
    }

    #[test]
    fn push_pop_preserves_order_and_aborts_run_stream() {
        let dir = tempdir().unwrap();
        let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
        app.push(Screen::Rooms(state::screens::RoomsState::new("ws_1".to_string())));
        assert_eq!(app.current().id(), ScreenId::Rooms);
        let popped = app.pop();
        assert!(matches!(popped, Some(Screen::Rooms(_))));
        assert_eq!(app.current().id(), ScreenId::Login);
    }
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p tui`
Expected: compile error — `cannot find type \`App\``.

- [ ] **Step 3: Write the implementation — full `crates/tui/src/app.rs`**

```rust
use crate::screens;
use api_client::{ControlPlaneApi, ControlPlaneError, RunEvent, RunResponse, WorkspaceSelection};
use crossterm::event::KeyEvent;
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::Paragraph;
use ratatui::{Frame, Terminal};
use state::screens::{RunState, Screen, ScreenId};
use state::session_store::{SessionData, SessionStore, StateError};
use std::io;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("terminal io error: {0}")]
    Io(#[from] io::Error),
    #[error("control plane error: {0}")]
    Api(#[from] ControlPlaneError),
    #[error("state error: {0}")]
    State(#[from] StateError),
}

/// A user command produced by a screen key handler; the async run loop
/// executes it against the api client and the screen stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    None,
    Quit,
    Back,
    SubmitLogin,
    CreateWorkspace,
    NavigateToRooms,
    RefreshRooms,
    NavigateToRoomBinding,
    BindRoom,
    NavigateToComposer,
    LaunchRun,
    CancelRun,
    RefreshDeliveries,
    NavigateToGitHubWorkspace,
    RefreshPanel,
    RequestGrant,
    ConfirmMutation,
}

/// Events produced by the run's SSE stream task.
#[derive(Debug)]
pub enum AppEvent {
    RunEvent(RunEvent),
    Reconnecting,
    RunStreamEnded,
    RunError(ControlPlaneError),
}

/// Error codes that mean the write gate refused a mutation (mobile DENIAL_CODES).
const DENIAL_CODES: &[&str] = &[
    "WRITE_SCOPE_REQUIRED",
    "APPROVAL_DENIED",
    "APPROVAL_MISMATCH",
    "APPROVAL_NOT_FOUND",
    "APPROVAL_CONFIRMATION_REQUIRED",
    "COMMAND_NOT_ALLOWED",
    "RUN_NOT_FOUND",
];

pub struct App {
    pub base_url: String,
    pub client: ControlPlaneApi,
    pub store: SessionStore,
    pub stack: Vec<Screen>,
    pub status: Option<String>,
    pub should_quit: bool,
    pub github_installation_id: Option<String>,
    stream_rx: Option<mpsc::UnboundedReceiver<AppEvent>>,
    stream_task: Option<JoinHandle<()>>,
}

impl App {
    /// Restore the stored session (if any) and pick the initial screen.
    pub fn new(base_url: String, store: SessionStore) -> Self {
        let data = store.load().unwrap_or_default();
        let mut client = ControlPlaneApi::new(&base_url).unwrap_or_else(|_| {
            ControlPlaneApi::new("http://localhost:3000").expect("constant base url is valid")
        });
        client.set_cookie(data.cookie.clone());
        let initial = if data.cookie.is_some() {
            let mut state = state::screens::WorkspacesState::new();
            for workspace in data.workspaces {
                state.add_workspace(workspace);
            }
            Screen::Workspaces(state)
        } else {
            Screen::Login(state::screens::LoginState::default())
        };
        let github_installation_id =
            std::env::var("MATRIX_WORKSPACE_TUI_GITHUB_INSTALLATION_ID").ok();
        Self {
            base_url,
            client,
            store,
            stack: vec![initial],
            status: None,
            should_quit: false,
            github_installation_id,
            stream_rx: None,
            stream_task: None,
        }
    }

    pub fn current(&self) -> &Screen {
        self.stack.last().expect("stack is never empty")
    }

    pub fn current_mut(&mut self) -> &mut Screen {
        self.stack.last_mut().expect("stack is never empty")
    }

    pub fn push(&mut self, screen: Screen) {
        self.stack.push(screen);
    }

    /// Pop the top screen. Leaving the Run screen aborts its SSE task.
    pub fn pop(&mut self) -> Option<Screen> {
        let popped = self.stack.pop();
        if matches!(popped, Some(Screen::Run(_))) {
            self.abort_stream();
        }
        popped
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status = Some(message.into());
    }

    /// 401 anywhere: clear the stored session and return to Login.
    pub fn expire_session(&mut self) {
        let _ = self.store.clear();
        self.client.set_cookie(None);
        self.stack = vec![Screen::Login(state::screens::LoginState::default())];
        self.set_status("Session expired; sign in again");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use state::screens::{Screen, ScreenId};
    use state::session_store::SessionStore;
    use tempfile::tempdir;

    #[test]
    fn app_starts_on_login_without_session() {
        let dir = tempdir().unwrap();
        let app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
        assert_eq!(app.current().id(), ScreenId::Login);
        assert_eq!(app.stack.len(), 1);
    }

    #[test]
    fn app_starts_on_workspaces_with_stored_session() {
        let dir = tempdir().unwrap();
        let store = SessionStore::at_path(dir.path().join("session.json"));
        let mut data = SessionData::default();
        data.cookie = Some("cp_session=abc123".to_string());
        data.workspaces.push(WorkspaceSelection {
            workspace_id: "ws_1".to_string(),
            name: "My workspace".to_string(),
            owner_id: "@u:example.org".to_string(),
            status: "active".to_string(),
            created_at: "2026-08-15T00:00:00.000Z".to_string(),
        });
        store.save(&data).unwrap();

        let app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
        assert_eq!(app.current().id(), ScreenId::Workspaces);
        assert_eq!(app.client.cookie(), Some("cp_session=abc123"));
        let Screen::Workspaces(state) = app.current() else { panic!("workspaces") };
        assert_eq!(state.workspaces.len(), 1);
    }

    #[test]
    fn push_pop_preserves_order_and_aborts_run_stream() {
        let dir = tempdir().unwrap();
        let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
        app.push(Screen::Rooms(state::screens::RoomsState::new("ws_1".to_string())));
        assert_eq!(app.current().id(), ScreenId::Rooms);
        let popped = app.pop();
        assert!(matches!(popped, Some(Screen::Rooms(_))));
        assert_eq!(app.current().id(), ScreenId::Login);
    }
}
```

- [ ] **Step 4: Declare the modules in `main.rs` and create the screens module**

`main.rs` is the binary root; the app + tests live in its modules. Replace the placeholder `crates/tui/src/main.rs` with:

```rust
mod app;
mod screens;

fn main() {
    eprintln!("matrix-workspace-tui: build placeholder");
    std::process::exit(1);
}
```

Create `crates/tui/src/screens/mod.rs`:

```rust
// Screen modules are stubbed in Task 7.2 and replaced with full
// implementations in Group 8.
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: `test result: ok. 3 passed; 0 failed; ...`

- [ ] **Step 6: Commit**

```bash
git add crates/tui/src/app.rs crates/tui/src/screens/mod.rs
git commit -m "feat(tui): app shell with screen stack and session restore"
```

### Task 7.2: Key dispatch + stub screen handlers

**Files:**
- Modify: `crates/tui/src/app.rs` (add `handle_key`)
- Create: `crates/tui/src/screens/login.rs`, `workspaces.rs`, `rooms.rs`, `run_composer.rs`, `run.rs`, `github.rs` (stubs)

- [ ] **Step 1: Write the failing tests (append to the `tests` module in `crates/tui/src/app.rs`)**

```rust
use crossterm::event::{KeyCode, KeyEvent};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, crossterm::event::KeyModifiers::NONE)
}

#[test]
fn login_key_q_quits_and_other_keys_are_noops() {
    let dir = tempdir().unwrap();
    let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
    assert_eq!(app.handle_key(key(KeyCode::Char('q'))), Command::Quit);
    assert_eq!(app.handle_key(key(KeyCode::Char('x'))), Command::None);
}

#[test]
fn workspaces_key_q_quits_at_root() {
    let dir = tempdir().unwrap();
    let store = SessionStore::at_path(dir.path().join("session.json"));
    let mut data = SessionData::default();
    data.cookie = Some("cp_session=abc123".to_string());
    store.save(&data).unwrap();
    let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
    assert_eq!(app.handle_key(key(KeyCode::Char('q'))), Command::Quit);
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p tui login_key_q_quits_and_other_keys_are_noops`
Expected: compile error — `cannot find function \`handle_key\``.

- [ ] **Step 3: Write the implementation**

Add to `crates/tui/src/app.rs` (inside `impl App`):

```rust
    /// Route a key event to the active screen's handler.
    pub fn handle_key(&mut self, key: KeyEvent) -> Command {
        match self.current_mut() {
            Screen::Login(state) => screens::login::handle_login_key(state, key),
            Screen::Workspaces(state) => screens::workspaces::handle_workspaces_key(state, key),
            Screen::Rooms(state) => screens::rooms::handle_rooms_key(state, key),
            Screen::RoomBinding(state) => screens::rooms::handle_room_binding_key(state, key),
            Screen::RunComposer(state) => screens::run_composer::handle_run_composer_key(state, key),
            Screen::Run(state) => screens::run::handle_run_key(state, key),
            Screen::GitHubWorkspace(state) => screens::github::handle_github_workspace_key(state, key),
        }
    }
```

Create the six stub modules. `crates/tui/src/screens/login.rs`:

```rust
use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::LoginState;

/// Stub — replaced by the full handler in Group 8, Task 8.1.
pub fn handle_login_key(_state: &mut LoginState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Quit,
        _ => Command::None,
    }
}

/// Stub — replaced by the full render in Group 8, Task 8.2.
pub fn render_login<B: Backend>(_state: &LoginState, _frame: &mut Frame<B>, _area: Rect) {}
```

`crates/tui/src/screens/workspaces.rs`:

```rust
use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::WorkspacesState;

/// Stub — replaced in Group 8, Task 8.3.
pub fn handle_workspaces_key(_state: &mut WorkspacesState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Quit,
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.4.
pub fn render_workspaces<B: Backend>(_state: &WorkspacesState, _frame: &mut Frame<B>, _area: Rect) {}
```

`crates/tui/src/screens/rooms.rs`:

```rust
use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::{RoomBindingState, RoomsState};

/// Stub — replaced in Group 8, Task 8.5.
pub fn handle_rooms_key(_state: &mut RoomsState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.6.
pub fn render_rooms<B: Backend>(_state: &RoomsState, _frame: &mut Frame<B>, _area: Rect) {}

/// Stub — replaced in Group 8, Task 8.5.
pub fn handle_room_binding_key(_state: &mut RoomBindingState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.6.
pub fn render_room_binding<B: Backend>(_state: &RoomBindingState, _frame: &mut Frame<B>, _area: Rect) {}
```

`crates/tui/src/screens/run_composer.rs`:

```rust
use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::RunComposerState;

/// Stub — replaced in Group 8, Task 8.7.
pub fn handle_run_composer_key(_state: &mut RunComposerState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.8.
pub fn render_run_composer<B: Backend>(_state: &RunComposerState, _frame: &mut Frame<B>, _area: Rect) {}
```

`crates/tui/src/screens/run.rs`:

```rust
use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::RunState;

/// Stub — replaced in Group 8, Task 8.9.
pub fn handle_run_key(_state: &mut RunState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.10.
pub fn render_run<B: Backend>(_state: &RunState, _frame: &mut Frame<B>, _area: Rect) {}
```

`crates/tui/src/screens/github.rs`:

```rust
use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::backend::Backend;
use ratatui::layout::Rect;
use ratatui::Frame;
use state::screens::GitHubWorkspaceState;

/// Stub — replaced in Group 8, Task 8.11.
pub fn handle_github_workspace_key(_state: &mut GitHubWorkspaceState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        _ => Command::None,
    }
}

/// Stub — replaced in Group 8, Task 8.12.
pub fn render_github_workspace<B: Backend>(_state: &GitHubWorkspaceState, _frame: &mut Frame<B>, _area: Rect) {}
```

Update `crates/tui/src/screens/mod.rs`:

```rust
pub mod github;
pub mod login;
pub mod rooms;
pub mod run;
pub mod run_composer;
pub mod workspaces;
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: `test result: ok. 5 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/app.rs crates/tui/src/screens
git commit -m "feat(tui): key dispatch with stub screen handlers"
```

### Task 7.3: `execute_command` — synchronous commands (Quit/Back/NavigateToRoomBinding/NavigateToComposer)

**Files:**
- Modify: `crates/tui/src/app.rs`

- [ ] **Step 1: Write the failing tests (append to the `tests` module)**

```rust
#[tokio::test]
async fn back_command_pops_to_previous_screen() {
    let dir = tempdir().unwrap();
    let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
    app.push(Screen::Rooms(state::screens::RoomsState::new("ws_1".to_string())));
    app.execute_command(Command::Back).await;
    assert_eq!(app.current().id(), ScreenId::Login);
}

#[tokio::test]
async fn back_at_root_quits() {
    let dir = tempdir().unwrap();
    let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
    app.execute_command(Command::Back).await;
    assert!(app.should_quit);
}

#[tokio::test]
async fn navigate_to_room_binding_pushes_binding_screen() {
    let dir = tempdir().unwrap();
    let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
    let mut rooms = state::screens::RoomsState::new("ws_1".to_string());
    rooms.set_rooms(vec![api_client::RoomSummary {
        room_id: "!a:example.org".to_string(),
        homeserver_url: "https://example.org".to_string(),
        display_name: None,
        workspace_id: None,
    }]);
    app.push(Screen::Rooms(rooms));
    app.execute_command(Command::NavigateToRoomBinding).await;
    assert_eq!(app.current().id(), ScreenId::RoomBinding);
}

#[tokio::test]
async fn navigate_to_composer_pushes_composer_screen() {
    let dir = tempdir().unwrap();
    let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
    let mut rooms = state::screens::RoomsState::new("ws_1".to_string());
    rooms.set_rooms(vec![api_client::RoomSummary {
        room_id: "!a:example.org".to_string(),
        homeserver_url: "https://example.org".to_string(),
        display_name: None,
        workspace_id: Some("ws_1".to_string()),
    }]);
    app.push(Screen::Rooms(rooms));
    app.execute_command(Command::NavigateToComposer).await;
    assert_eq!(app.current().id(), ScreenId::RunComposer);
    let Screen::RunComposer(composer) = app.current() else { panic!("composer") };
    assert_eq!(composer.room_id, "!a:example.org");
    assert_eq!(composer.workspace_id, "ws_1");
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p tui back_command_pops_to_previous_screen`
Expected: compile error — `cannot find function \`execute_command\``.

- [ ] **Step 3: Write the implementation (append to `crates/tui/src/app.rs`; add `impl App` block if needed)**

```rust
impl App {
    /// Execute a command produced by a screen handler.
    pub async fn execute_command(&mut self, command: Command) {
        match command {
            Command::None => {}
            Command::Quit => self.should_quit = true,
            Command::Back => {
                if self.stack.len() > 1 {
                    self.pop();
                } else {
                    self.should_quit = true;
                }
            }
            Command::NavigateToRoomBinding => self.navigate_to_room_binding(),
            Command::NavigateToComposer => self.navigate_to_composer(),
            Command::NavigateToRooms => self.navigate_to_rooms().await,
            Command::NavigateToGitHubWorkspace => self.navigate_to_github_workspace(),
            Command::SubmitLogin => self.submit_login().await,
            Command::CreateWorkspace => self.create_workspace().await,
            Command::RefreshRooms => self.refresh_rooms().await,
            Command::BindRoom => self.bind_room().await,
            Command::LaunchRun => self.launch_run().await,
            Command::CancelRun => self.cancel_run().await,
            Command::RefreshDeliveries => self.refresh_deliveries().await,
            Command::RefreshPanel => self.refresh_github_panel().await,
            Command::RequestGrant => self.request_grant().await,
            Command::ConfirmMutation => self.confirm_mutation().await,
        }
    }

    fn navigate_to_room_binding(&mut self) {
        let (room, workspace_id) = match self.current() {
            Screen::Rooms(state) => match state.selected_room() {
                Some(room) => (room.clone(), state.workspace_id.clone()),
                None => return,
            },
            _ => return,
        };
        self.push(Screen::RoomBinding(state::screens::RoomBindingState::new(room, workspace_id)));
    }

    fn navigate_to_composer(&mut self) {
        let (room_id, workspace_id) = match self.current() {
            Screen::Rooms(state) => match state.selected_room() {
                Some(room) => (room.room_id.clone(), state.workspace_id.clone()),
                None => return,
            },
            _ => return,
        };
        self.push(Screen::RunComposer(state::screens::RunComposerState::new(room_id, workspace_id)));
    }

    fn navigate_to_github_workspace(&mut self) {
        let (workspace_id, run_id) = match self.current() {
            Screen::Run(state) => (state.workspace_id.clone(), state.run_id.clone()),
            _ => return,
        };
        let installation_id = self.github_installation_id.clone();
        self.push(Screen::GitHubWorkspace(
            state::screens::GitHubWorkspaceState::new(workspace_id, run_id, installation_id),
        ));
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: FAIL still — `cannot find function \`navigate_to_rooms\`` and the other async methods (referenced by `execute_command`). They land in Task 7.4. To keep this task green, comment out the async arms of `execute_command` (SubmitLogin/CreateWorkspace/NavigateToRooms/RefreshRooms/BindRoom/LaunchRun/CancelRun/RefreshDeliveries/RefreshPanel/RequestGrant/ConfirmMutation) until Task 7.4 adds them, then restore. The four sync tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/app.rs
git commit -m "feat(tui): command executor with navigation"
```

### Task 7.4: Async executor — login, workspace, rooms, bind, run launch + SSE stream, session expiry

**Files:**
- Modify: `crates/tui/src/app.rs`

- [ ] **Step 1: Write the failing tests (append to the `tests` module)**

```rust
use httpmock::prelude::*;
use serde_json::json;

#[tokio::test]
async fn submit_login_stores_cookie_and_opens_workspaces() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(POST).path("/api/auth/matrix/session");
            then.status(200)
                .header("set-cookie", "cp_session=abc123; Path=/")
                .json_body(json!({
                    "user": { "id": "@u:matrix.example.org", "homeserverUrl": "https://matrix.example.org" },
                    "sessionExpiresAt": "2026-08-15T01:00:00.000Z"
                }));
        })
        .await;

    let dir = tempdir().unwrap();
    let mut app = App::new(server.base_url(), SessionStore::at_path(dir.path().join("session.json")));
    let Screen::Login(state) = app.current_mut() else { panic!("login") };
    state.set_homeserver_url("https://matrix.example.org".to_string());
    state.set_access_token("tok_1".to_string());

    app.execute_command(Command::SubmitLogin).await;

    assert_eq!(app.current().id(), ScreenId::Workspaces);
    assert_eq!(app.client.cookie(), Some("cp_session=abc123"));
    let stored = SessionStore::at_path(dir.path().join("session.json")).load().unwrap();
    assert_eq!(stored.cookie.as_deref(), Some("cp_session=abc123"));
}

#[tokio::test]
async fn create_workspace_appends_and_persists() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(POST).path("/api/workspaces");
            then.status(201).json_body(json!({
                "requestId": "req_1",
                "workspaceId": "ws_new",
                "name": "ops",
                "ownerId": "@u:matrix.example.org",
                "status": "active",
                "createdAt": "2026-08-15T00:00:00.000Z"
            }));
        })
        .await;

    let dir = tempdir().unwrap();
    let store = SessionStore::at_path(dir.path().join("session.json"));
    let mut data = SessionData::default();
    data.cookie = Some("cp_session=abc123".to_string());
    store.save(&data).unwrap();

    let mut app = App::new(server.base_url(), SessionStore::at_path(dir.path().join("session.json")));
    let Screen::Workspaces(state) = app.current_mut() else { panic!("workspaces") };
    state.creating = true;
    state.set_name_input("ops".to_string());

    app.execute_command(Command::CreateWorkspace).await;

    let Screen::Workspaces(state) = app.current() else { panic!("workspaces") };
    assert_eq!(state.workspaces.len(), 1);
    assert_eq!(state.workspaces[0].workspace_id, "ws_new");
    let stored = SessionStore::at_path(dir.path().join("session.json")).load().unwrap();
    assert_eq!(stored.workspaces.len(), 1);
    assert_eq!(stored.workspaces[0].name, "ops");
}

#[tokio::test]
async fn session_expired_anywhere_clears_session_and_returns_to_login() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/api/rooms");
            then.status(401);
        })
        .await;

    let dir = tempdir().unwrap();
    let store = SessionStore::at_path(dir.path().join("session.json"));
    let mut data = SessionData::default();
    data.cookie = Some("cp_session=stale".to_string());
    store.save(&data).unwrap();

    let mut app = App::new(server.base_url(), SessionStore::at_path(dir.path().join("session.json")));
    app.push(Screen::Rooms(state::screens::RoomsState::new("ws_1".to_string())));

    app.execute_command(Command::RefreshRooms).await;

    assert_eq!(app.current().id(), ScreenId::Login);
    assert_eq!(app.client.cookie(), None);
    let stored = SessionStore::at_path(dir.path().join("session.json")).load().unwrap();
    assert_eq!(stored.cookie, None, "stale session cleared from disk");
    assert!(app.status.as_deref().unwrap_or("").contains("Session expired"));
}

#[tokio::test]
async fn launch_run_opens_run_screen_and_streams_terminal_event() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(POST).path("/api/workspaces/ws_1/runs");
            then.status(202).json_body(json!({
                "runId": "r1",
                "status": "queued",
                "roomId": "!a:example.org",
                "nextSequence": 1
            }));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/api/runs/r1/events")
                .query_param("after", "1");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(
                    // `after=1` means the server replays events AFTER sequence 1,
                    // so the mocked stream starts at sequence 2.
                    "id: 2\nevent: run.queued\ndata: {\"id\":\"ev_2\",\"runId\":\"r1\",\"sequence\":2,\"type\":\"run.queued\",\"version\":1,\"occurredAt\":\"2026-08-15T00:00:00.000Z\",\"visibility\":\"room_and_owner\",\"payload\":{}}\n\n"
                        + "id: 3\nevent: run.completed\ndata: {\"id\":\"ev_3\",\"runId\":\"r1\",\"sequence\":3,\"type\":\"run.completed\",\"version\":1,\"occurredAt\":\"2026-08-15T00:00:00.000Z\",\"visibility\":\"room_and_owner\",\"payload\":{}}\n\n",
                );
        })
        .await;

    let dir = tempdir().unwrap();
    let store = SessionStore::at_path(dir.path().join("session.json"));
    let mut data = SessionData::default();
    data.cookie = Some("cp_session=abc123".to_string());
    store.save(&data).unwrap();

    let mut app = App::new(server.base_url(), SessionStore::at_path(dir.path().join("session.json")));
    let mut composer = state::screens::RunComposerState::new("!a:example.org".to_string(), "ws_1".to_string());
    composer.set_prompt("Go".to_string());
    composer.toggle_mode(api_client::RunMode::Parallel);
    composer.toggle_specialist("repo-reader");
    app.push(Screen::RunComposer(composer));

    app.execute_command(Command::LaunchRun).await;
    assert_eq!(app.current().id(), ScreenId::Run);

    // Give the SSE task a moment to deliver the terminal event.
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    app.drain_stream_events();

    let Screen::Run(state) = app.current() else { panic!("run") };
    assert_eq!(state.run_id, "r1");
    assert!(state.is_terminal());
    assert_eq!(state.events().len(), 2);
    assert_eq!(state.highest_sequence(), 3);
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p tui submit_login_stores_cookie_and_opens_workspaces`
Expected: FAIL — the `Command::SubmitLogin` arm of `execute_command` is commented out (Task 7.3), so the command silently does nothing → `test result: FAILED` with `left: Login / right: Workspaces`. Good red.

- [ ] **Step 3: Write the implementation (append to the second `impl App` block in `crates/tui/src/app.rs`; restore the commented arms in `execute_command` first)**

```rust
    async fn submit_login(&mut self) {
        let (homeserver_url, access_token) = match self.current() {
            Screen::Login(state) => (
                state.homeserver_url.trim().to_string(),
                state.access_token.trim().to_string(),
            ),
            _ => return,
        };
        match self.client.create_matrix_session(&homeserver_url, &access_token).await {
            Ok(_) => {
                let cookie = self.client.cookie().map(|value| value.to_string());
                let data = SessionData {
                    cookie,
                    workspaces: Vec::new(),
                };
                if let Err(error) = self.store.save(&data) {
                    self.set_status(format!("Could not save session: {error}"));
                    return;
                }
                self.push(Screen::Workspaces(state::screens::WorkspacesState::new()));
                self.set_status("Signed in");
            }
            Err(error) => {
                if let Screen::Login(state) = self.current_mut() {
                    state.error = Some(error.to_string());
                }
            }
        }
    }

    async fn create_workspace(&mut self) {
        let name = match self.current() {
            Screen::Workspaces(state) => state.name_input.trim().to_string(),
            _ => return,
        };
        if name.is_empty() {
            self.set_status("Workspace name is required");
            return;
        }
        match self.client.create_workspace(&name).await {
            Ok(workspace) => {
                if let Screen::Workspaces(state) = self.current_mut() {
                    state.add_workspace(workspace.clone());
                    state.creating = false;
                    state.set_name_input(String::new());
                }
                let mut data = self.store.load().unwrap_or_default();
                data.workspaces.push(workspace);
                match self.store.save(&data) {
                    Ok(()) => self.set_status("Workspace created"),
                    Err(error) => self.set_status(format!("Could not persist workspace: {error}")),
                }
            }
            Err(error) => {
                if error.is_session_expired() {
                    self.expire_session();
                    return;
                }
                if let Screen::Workspaces(state) = self.current_mut() {
                    state.error = Some(error.to_string());
                }
            }
        }
    }

    async fn navigate_to_rooms(&mut self) {
        let workspace_id = match self.current() {
            Screen::Workspaces(state) => state.selected().map(|workspace| workspace.workspace_id.clone()),
            _ => None,
        };
        let Some(workspace_id) = workspace_id else {
            return;
        };
        self.push(Screen::Rooms(state::screens::RoomsState::new(workspace_id.clone())));
        if let Screen::Rooms(state) = self.current_mut() {
            state.loading = true;
        }
        self.refresh_rooms().await;
    }

    async fn refresh_rooms(&mut self) {
        match self.client.get_rooms().await {
            Ok(rooms) => {
                if let Screen::Rooms(state) = self.current_mut() {
                    state.set_rooms(rooms);
                    state.loading = false;
                }
            }
            Err(error) => {
                if error.is_session_expired() {
                    self.expire_session();
                    return;
                }
                if let Screen::Rooms(state) = self.current_mut() {
                    state.error = Some(error.to_string());
                    state.loading = false;
                }
            }
        }
    }

    async fn bind_room(&mut self) {
        let (room_id, workspace_id) = match self.current() {
            Screen::RoomBinding(state) => (state.room.room_id.clone(), state.workspace_id.clone()),
            _ => return,
        };
        match self.client.bind_room(&room_id, &workspace_id).await {
            Ok(_) => {
                if let Screen::RoomBinding(state) = self.current_mut() {
                    state.mark_bound();
                }
                if let Screen::RoomBinding(state) = self.current() {
                    if state.done {
                        self.pop();
                        if let Screen::Rooms(state) = self.current_mut() {
                            state.mark_room_bound(&room_id);
                        }
                        self.set_status("Room bound");
                    }
                }
            }
            Err(error) => {
                if error.is_session_expired() {
                    self.expire_session();
                    return;
                }
                if let Screen::RoomBinding(state) = self.current_mut() {
                    state.error = Some(error.to_string());
                }
            }
        }
    }

    async fn launch_run(&mut self) {
        let (request, workspace_id) = match self.current() {
            Screen::RunComposer(state) => match state.request() {
                Some(request) => (request, state.workspace_id.clone()),
                None => return,
            },
            _ => return,
        };
        let idempotency_key = uuid::Uuid::new_v4().to_string();
        match self.client.launch_run(&workspace_id, &request, &idempotency_key).await {
            Ok(run) => {
                self.enter_run(run, workspace_id);
                self.set_status("Run launched");
            }
            Err(error) => {
                if error.is_session_expired() {
                    self.expire_session();
                    return;
                }
                if let Screen::RunComposer(state) = self.current_mut() {
                    state.error = Some(error.to_string());
                }
            }
        }
    }

    /// Push the Run screen and start the SSE stream task.
    fn enter_run(&mut self, run: RunResponse, workspace_id: String) {
        let run_id = run.run_id.clone();
        let after = run.next_sequence;
        let cookie = self.client.cookie().unwrap_or("").to_string();
        let base_url = self.base_url.clone();
        let (tx, rx) = mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            use api_client::sse::{EventStream, StreamEvent};
            let mut stream = EventStream::new(&base_url, &cookie, &run_id, after);
            loop {
                match stream.next().await {
                    Some(Ok(StreamEvent::Run(event))) => {
                        if tx.send(AppEvent::RunEvent(event)).is_err() {
                            break;
                        }
                    }
                    Some(Ok(StreamEvent::Reconnecting { .. })) => {
                        if tx.send(AppEvent::Reconnecting).is_err() {
                            break;
                        }
                    }
                    Some(Err(error)) => {
                        let _ = tx.send(AppEvent::RunError(error));
                        break;
                    }
                    None => {
                        let _ = tx.send(AppEvent::RunStreamEnded);
                        break;
                    }
                }
            }
        });
        self.stream_rx = Some(rx);
        self.stream_task = Some(task);
        self.push(Screen::Run(RunState::new(run_id, workspace_id)));
    }

    /// Drain pending stream events into the Run screen state.
    pub fn drain_stream_events(&mut self) {
        let Some(rx) = &mut self.stream_rx else {
            return;
        };
        while let Ok(event) = rx.try_recv() {
            match self.current_mut() {
                Screen::Run(state) => match event {
                    AppEvent::RunEvent(event) => {
                        state.set_reconnecting(false);
                        state.accept(event);
                    }
                    AppEvent::Reconnecting => state.set_reconnecting(true),
                    AppEvent::RunStreamEnded => {
                        // The server closed the stream (terminal run or end of
                        // replay). The terminal state is already visible via
                        // the accepted events.
                    }
                    AppEvent::RunError(error) => {
                        if error.is_session_expired() {
                            self.expire_session();
                            return;
                        }
                        state.error = Some(error.to_string());
                    }
                },
                _ => return,
            }
        }
    }

    fn abort_stream(&mut self) {
        if let Some(task) = self.stream_task.take() {
            task.abort();
        }
        self.stream_rx = None;
    }
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: `test result: ok. 13 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/app.rs
git commit -m "feat(tui): async command executor with run stream and session expiry"
```

### Task 7.5: Async executor — cancel, deliveries, GitHub read panels, grant, mutation confirm

**Files:**
- Modify: `crates/tui/src/app.rs`

- [ ] **Step 1: Write the failing tests (append to the `tests` module)**

```rust
#[tokio::test]
async fn cancel_run_marks_cancel_requested() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(POST).path("/api/runs/r1/cancel");
            then.status(202).json_body(json!({
                "requestId": "req_1",
                "runId": "r1",
                "status": "cancellation_requested"
            }));
        })
        .await;

    let dir = tempdir().unwrap();
    let store = SessionStore::at_path(dir.path().join("session.json"));
    let mut data = SessionData::default();
    data.cookie = Some("cp_session=abc123".to_string());
    store.save(&data).unwrap();

    let mut app = App::new(server.base_url(), SessionStore::at_path(dir.path().join("session.json")));
    app.push(Screen::Run(state::screens::RunState::new("r1".to_string(), "ws_1".to_string())));

    app.execute_command(Command::CancelRun).await;

    let Screen::Run(state) = app.current() else { panic!("run") };
    assert!(state.cancel_requested);
}

#[tokio::test]
async fn refresh_deliveries_reads_authoritative_status() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(GET).path("/api/runs/r1");
            then.status(200).json_body(json!({
                "requestId": "req_1",
                "runId": "r1",
                "status": "running",
                "mode": "parallel",
                "workspaceId": "ws_1",
                "roomId": null,
                "specialists": [],
                "lastSequence": 5,
                "matrixDeliveries": [
                    { "sequence": 1, "status": "delivered" },
                    { "sequence": 2, "status": "pending" }
                ],
                "cancelRequestedAt": null
            }));
        })
        .await;

    let dir = tempdir().unwrap();
    let store = SessionStore::at_path(dir.path().join("session.json"));
    let mut data = SessionData::default();
    data.cookie = Some("cp_session=abc123".to_string());
    store.save(&data).unwrap();

    let mut app = App::new(server.base_url(), SessionStore::at_path(dir.path().join("session.json")));
    app.push(Screen::Run(state::screens::RunState::new("r1".to_string(), "ws_1".to_string())));

    app.execute_command(Command::RefreshDeliveries).await;

    let Screen::Run(state) = app.current() else { panic!("run") };
    assert_eq!(state.deliveries.len(), 2);
    assert_eq!(state.deliveries[0].status, api_client::MatrixDeliveryStatus::Delivered);
}

#[tokio::test]
async fn confirm_mutation_records_approval_and_queues_command() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(POST).path("/api/runs/r1/approvals")
                .body_contains(r#""approvalType":"github_mutation""#)
                .body_contains(r#""decision":"approved""#)
                .body_contains(r#""confirmationText":"I confirm create issue on octo/repo (issues:write)""#);
            then.status(200).json_body(json!({
                "approvalId": "apr_1",
                "status": "approved",
                "expiresAt": "2026-08-15T01:00:00.000Z",
                "scope": "issues:write"
            }));
        })
        .await;
    server
        .mock_async(|when, then| {
            when.method(POST).path("/api/workspaces/ws_1/github/mutations")
                .body_contains(r#""operation":"create_issue""#)
                .body_contains(r#""repository":"octo/repo""#);
            then.status(202).json_body(json!({
                "commandId": "cmd_1",
                "status": "queued"
            }));
        })
        .await;

    let dir = tempdir().unwrap();
    let store = SessionStore::at_path(dir.path().join("session.json"));
    let mut data = SessionData::default();
    data.cookie = Some("cp_session=abc123".to_string());
    store.save(&data).unwrap();

    let mut app = App::new(server.base_url(), SessionStore::at_path(dir.path().join("session.json")));
    let mut github = state::screens::GitHubWorkspaceState::new("ws_1".to_string(), "r1".to_string(), Some("inst_9".to_string()));
    github.set_repositories(vec![api_client::GithubRepositorySummary {
        id: 1,
        name: "repo".to_string(),
        full_name: "octo/repo".to_string(),
        owner: "octo".to_string(),
        private: false,
        default_branch: "main".to_string(),
        description: None,
        html_url: "https://github.com/octo/repo".to_string(),
        archived: false,
    }]);
    let draft = github.begin_mutation("Test issue".to_string()).unwrap();
    github.confirmation = Some(draft);
    app.push(Screen::GitHubWorkspace(github));

    app.execute_command(Command::ConfirmMutation).await;

    let Screen::GitHubWorkspace(state) = app.current() else { panic!("github") };
    assert_eq!(state.mutation_status, state::screens::MutationFlowStatus::Submitted);
    assert_eq!(state.command_id.as_deref(), Some("cmd_1"));
    assert!(state.confirmation.is_none(), "confirmation consumed after enqueue");
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p tui confirm_mutation_records_approval_and_queues_command`
Expected: FAIL — the `Command::ConfirmMutation` arm is still missing → the command silently does nothing → `test result: FAILED` with `left: Idle / right: Submitted`.

- [ ] **Step 3: Write the implementation (append to the same `impl App` block)**

```rust
    async fn cancel_run(&mut self) {
        let run_id = match self.current() {
            Screen::Run(state) => state.run_id.clone(),
            _ => return,
        };
        match self.client.cancel_run(&run_id).await {
            Ok(_) => {
                if let Screen::Run(state) = self.current_mut() {
                    state.request_cancel();
                }
                self.set_status("Cancellation requested");
            }
            Err(error) => {
                if error.is_session_expired() {
                    self.expire_session();
                    return;
                }
                if let Screen::Run(state) = self.current_mut() {
                    state.error = Some(error.to_string());
                }
            }
        }
    }

    async fn refresh_deliveries(&mut self) {
        let run_id = match self.current() {
            Screen::Run(state) => state.run_id.clone(),
            _ => return,
        };
        match self.client.get_run_matrix_deliveries(&run_id).await {
            Ok(deliveries) => {
                if let Screen::Run(state) = self.current_mut() {
                    state.set_deliveries(deliveries.deliveries);
                }
            }
            Err(error) => {
                if error.is_session_expired() {
                    self.expire_session();
                    return;
                }
                if let Screen::Run(state) = self.current_mut() {
                    state.error = Some(error.to_string());
                }
            }
        }
    }

    async fn refresh_github_panel(&mut self) {
        let (workspace_id, installation_id) = match self.current() {
            Screen::GitHubWorkspace(state) => {
                (state.workspace_id.clone(), state.installation_id.clone())
            }
            _ => return,
        };
        let Some(installation_id) = installation_id else {
            if let Screen::GitHubWorkspace(state) = self.current_mut() {
                state.error = Some(
                    "GitHub App not configured: set MATRIX_WORKSPACE_TUI_GITHUB_INSTALLATION_ID"
                        .to_string(),
                );
            }
            return;
        };
        let panel = match self.current() {
            Screen::GitHubWorkspace(state) => state.panel,
            _ => return,
        };
        match panel {
            state::screens::GithubPanel::Repositories => {
                match self
                    .client
                    .list_github_repositories(&workspace_id, &installation_id, None)
                    .await
                {
                    Ok(page) => {
                        if let Screen::GitHubWorkspace(state) = self.current_mut() {
                            state.set_repositories(page.items);
                        }
                    }
                    Err(error) => self.github_error(error),
                }
            }
            state::screens::GithubPanel::Issues => {
                let Some((owner, repo)) = self.selected_repo_split() else {
                    self.set_status("Select a repository first");
                    return;
                };
                match self
                    .client
                    .list_github_issues(&workspace_id, &installation_id, &owner, &repo, None)
                    .await
                {
                    Ok(page) => {
                        if let Screen::GitHubWorkspace(state) = self.current_mut() {
                            state.set_issues(page.items);
                        }
                    }
                    Err(error) => self.github_error(error),
                }
            }
            state::screens::GithubPanel::PullRequests => {
                let Some((owner, repo)) = self.selected_repo_split() else {
                    self.set_status("Select a repository first");
                    return;
                };
                match self
                    .client
                    .list_github_pull_requests(&workspace_id, &installation_id, &owner, &repo, None)
                    .await
                {
                    Ok(page) => {
                        if let Screen::GitHubWorkspace(state) = self.current_mut() {
                            state.set_pull_requests(page.items);
                        }
                    }
                    Err(error) => self.github_error(error),
                }
            }
            state::screens::GithubPanel::Audit => {
                match self.client.list_audit_records(&workspace_id, None).await {
                    Ok(page) => {
                        if let Screen::GitHubWorkspace(state) = self.current_mut() {
                            state.set_audit(page.items);
                        }
                    }
                    Err(error) => self.github_error(error),
                }
            }
        }
    }

    fn selected_repo_split(&self) -> Option<(String, String)> {
        let repository = match self.current() {
            Screen::GitHubWorkspace(state) => state.selected_repository(),
            _ => None,
        }?;
        let (owner, repo) = repository.split_once('/')?;
        Some((owner.to_string(), repo.to_string()))
    }

    fn github_error(&mut self, error: ControlPlaneError) {
        if error.is_session_expired() {
            self.expire_session();
            return;
        }
        if let Screen::GitHubWorkspace(state) = self.current_mut() {
            state.error = Some(error.to_string());
        }
    }

    async fn request_grant(&mut self) {
        let repository = match self.current() {
            Screen::GitHubWorkspace(state) => state.selected_repository(),
            _ => None,
        };
        let Some(repository) = repository else {
            self.set_status("Select a repository first");
            return;
        };
        let workspace_id = match self.current() {
            Screen::GitHubWorkspace(state) => state.workspace_id.clone(),
            _ => return,
        };
        match self
            .client
            .request_github_write_grant(&workspace_id, &repository, api_client::GithubWriteScope::IssuesWrite)
            .await
        {
            Ok(grant) => {
                if let Screen::GitHubWorkspace(state) = self.current_mut() {
                    state.set_grant(grant);
                }
                self.set_status("Write grant requested (pending approval)");
            }
            Err(error) => self.github_error(error),
        }
    }

    async fn confirm_mutation(&mut self) {
        use api_client::{
            ApprovalDecision, ApprovalType, CreateApprovalRequest, EnqueueMutationRequest,
        };
        use state::screens::{confirmation_sentence, MutationFlowStatus};

        let (draft, workspace_id, run_id) = match self.current() {
            Screen::GitHubWorkspace(state) => match &state.confirmation {
                Some(draft) => (draft.clone(), state.workspace_id.clone(), state.run_id.clone()),
                None => return,
            },
            _ => return,
        };
        // 1. Record the explicit approval (only ever here, on the confirm action).
        let approval_request = CreateApprovalRequest {
            approval_type: ApprovalType::GithubMutation,
            scope: draft.scope,
            decision: ApprovalDecision::Approved,
            confirmation_text: confirmation_sentence(draft.operation, &draft.repository, draft.scope),
            command_hash: draft.command_hash.clone(),
        };
        let approval = match self.client.create_run_approval(&run_id, &approval_request).await {
            Ok(approval) => approval,
            Err(error) => {
                self.github_error(error);
                return;
            }
        };
        // 2. Enqueue the mutation with the same idempotency key.
        let enqueue_request = EnqueueMutationRequest {
            idempotency_key: draft.idempotency_key.clone(),
            approval_id: approval.approval_id,
            repository: draft.repository.clone(),
            run_id: Some(run_id),
            operation: draft.operation,
            arguments: draft.arguments.clone(),
        };
        match self
            .client
            .enqueue_github_mutation(&workspace_id, &enqueue_request)
            .await
        {
            Ok(result) => {
                if let Screen::GitHubWorkspace(state) = self.current_mut() {
                    state.set_command_id(Some(result.command_id.clone()));
                    let status = if result.replayed {
                        MutationFlowStatus::Duplicate
                    } else {
                        match result.status {
                            api_client::MutationStatus::Queued => MutationFlowStatus::Submitted,
                            api_client::MutationStatus::Completed => MutationFlowStatus::Succeeded,
                            api_client::MutationStatus::Failed => MutationFlowStatus::Failed,
                        }
                    };
                    state.set_mutation_status(status);
                    state.confirmation = None;
                }
                self.set_status("Mutation queued");
            }
            Err(error) => {
                let status = if error.code() == Some("APPROVAL_EXPIRED") {
                    MutationFlowStatus::Expired
                } else if DENIAL_CODES.contains(&error.code().unwrap_or("")) {
                    MutationFlowStatus::Denied
                } else {
                    MutationFlowStatus::Failed
                };
                if let Screen::GitHubWorkspace(state) = self.current_mut() {
                    state.set_mutation_status(status);
                }
            }
        }
    }
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: `test result: ok. 16 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/app.rs
git commit -m "feat(tui): run controls and github workspace command executor"
```

### Task 7.6: Run loop, status bar, router render, and `main.rs`

**Files:**
- Modify: `crates/tui/src/app.rs` (add `run` + `draw`)
- Modify: `crates/tui/src/main.rs` (full binary)

- [ ] **Step 1: Write the failing tests (append to the `tests` module in `crates/tui/src/app.rs`)**

```rust
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn draw_shows_status_bar_and_renders_current_screen() {
    let dir = tempdir().unwrap();
    let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
    app.set_status("hello status");
    let backend = TestBackend::new(40, 4);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let rendered: String = terminal.backend().buffer().content().iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    assert!(rendered.contains("hello status"), "status bar rendered: {rendered}");
}

#[test]
fn draw_renders_run_screen_without_panicking() {
    let dir = tempdir().unwrap();
    let mut app = App::new("http://localhost:3000".to_string(), SessionStore::at_path(dir.path().join("session.json")));
    let mut run = state::screens::RunState::new("r1".to_string(), "ws_1".to_string());
    run.accept(api_client::RunEvent {
        id: "ev_1".to_string(),
        run_id: "r1".to_string(),
        sequence: 1,
        event_type: api_client::RunEventType::RunStarted,
        version: 1,
        occurred_at: "2026-08-15T00:00:00.000Z".to_string(),
        visibility: api_client::EventVisibility::RoomAndOwner,
        payload: serde_json::json!({}),
    });
    run.set_deliveries(vec![api_client::MatrixDelivery {
        sequence: 1,
        status: api_client::MatrixDeliveryStatus::Delivered,
    }]);
    app.push(Screen::Run(run));
    app.set_status("tracking run");

    let backend = TestBackend::new(60, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| app.draw(frame)).unwrap();
    let rendered: String = terminal.backend().buffer().content().iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    // The status bar must render regardless of which screen is active (the
    // Run screen render itself lands in Task 8.10).
    assert!(rendered.contains("tracking run"), "status bar rendered: {rendered}");
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p tui draw_shows_status_bar_and_renders_current_screen`
Expected: compile error — `cannot find function \`draw\``.

- [ ] **Step 3: Write the implementation (append to the `impl App` block)**

```rust
    /// Draw the current screen plus the one-line status bar.
    pub fn draw<B: Backend>(&mut self, frame: &mut Frame<B>) {
        let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(frame.area());
        match self.current_mut() {
            Screen::Login(state) => screens::login::render_login(state, frame, chunks[0]),
            Screen::Workspaces(state) => screens::workspaces::render_workspaces(state, frame, chunks[0]),
            Screen::Rooms(state) => screens::rooms::render_rooms(state, frame, chunks[0]),
            Screen::RoomBinding(state) => screens::rooms::render_room_binding(state, frame, chunks[0]),
            Screen::RunComposer(state) => screens::run_composer::render_run_composer(state, frame, chunks[0]),
            Screen::Run(state) => screens::run::render_run(state, frame, chunks[0]),
            Screen::GitHubWorkspace(state) => screens::github::render_github_workspace(state, frame, chunks[0]),
        }
        let status = self.status.as_deref().map(|message| format!(" {message}")).unwrap_or_default();
        frame.render_widget(Paragraph::new(status), chunks[1]);
    }

    /// The main event loop: poll keys, execute commands, drain stream events,
    /// redraw. Returns when the user quits.
    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), AppError> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;
            if self.should_quit {
                break;
            }
            if crossterm::event::poll(Duration::from_millis(100))? {
                let event = crossterm::event::read()?;
                match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        let command = self.handle_key(key);
                        self.execute_command(command).await;
                    }
                    Event::Paste(text) => self.handle_paste(&text),
                    _ => {}
                }
            }
            self.drain_stream_events();
            if self.should_quit {
                break;
            }
        }
        self.abort_stream();
        Ok(())
    }

    /// Bracketed paste: append into the active text field.
    fn handle_paste(&mut self, text: &str) {
        match self.current_mut() {
            Screen::Login(state) => state.insert_text(text),
            Screen::Workspaces(state) => {
                if state.creating {
                    let mut value = state.name_input.clone();
                    value.push_str(text);
                    state.set_name_input(value);
                }
            }
            Screen::RunComposer(state) => {
                let mut value = state.prompt.clone();
                value.push_str(text);
                state.set_prompt(value);
            }
            Screen::GitHubWorkspace(state) => {
                if state.mutation_mode {
                    let mut value = state.mutation_title.clone();
                    value.push_str(text);
                    state.set_mutation_title(value);
                }
            }
            _ => {}
        }
    }
```

Add the imports at the top of `crates/tui/src/app.rs`:

```rust
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use std::time::Duration;
```

- [ ] **Step 4: Replace `crates/tui/src/main.rs` with the real binary**

The `tui` crate is a **binary** crate (no `lib.rs`), so `main.rs` declares the modules and uses `crate::` paths:

```rust
mod app;
mod screens;

use crate::app::{App, AppError};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use state::session_store::SessionStore;
use std::io;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let base_url = std::env::var("MATRIX_WORKSPACE_TUI_CONTROL_PLANE_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    let path = SessionStore::default_path().map_err(AppError::State)?;
    let store = SessionStore::at_path(path);

    let mut terminal = setup_terminal()?;
    let outcome = App::new(base_url, store).run(&mut terminal).await;
    restore_terminal(&mut terminal)?;
    outcome
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>, AppError> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), AppError> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
```

- [ ] **Step 5: Run the full workspace test suite**

Run: `cargo test --workspace`
Expected: `test result: ok.` for api-client (52), state (23), tui (14); and `cargo build --workspace` finishes with `Finished`.

- [ ] **Step 6: Commit**

```bash
git add crates/tui/src/app.rs crates/tui/src/main.rs
git commit -m "feat(tui): run loop, status bar, and main entrypoint"
```

---

# Group 8: tui — screens

Each task **replaces the stub file created in Task 7.2 entirely** with a full key handler + render + tests. Rendering is tested with ratatui's `TestBackend` (flatten the buffer symbols into a String and assert substrings).

### Task 8.1: Login screen — key handler

**Files:**
- Replace: `crates/tui/src/screens/login.rs`

- [ ] **Step 1: Write the failing tests**

Replace the whole `crates/tui/src/screens/login.rs` with:

```rust
use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use state::screens::LoginField;
use state::screens::LoginState;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, crossterm::event::KeyModifiers::NONE)
}

#[test]
fn typing_goes_to_the_focused_field() {
    let mut state = LoginState::default();
    assert_eq!(handle_login_key(&mut state, key(KeyCode::Char('h'))), Command::None);
    assert_eq!(state.homeserver_url, "h");
    assert_eq!(handle_login_key(&mut state, key(KeyCode::Char('\t'))), Command::None);
    assert_eq!(state.focus, LoginField::AccessToken);
    assert_eq!(handle_login_key(&mut state, key(KeyCode::Char('t'))), Command::None);
    assert_eq!(state.access_token, "t");
    assert_eq!(handle_login_key(&mut state, key(KeyCode::Backspace)), Command::None);
    assert_eq!(state.access_token, "");
}

#[test]
fn enter_submits_only_when_valid() {
    let mut state = LoginState::default();
    assert_eq!(handle_login_key(&mut state, key(KeyCode::Enter)), Command::None);
    assert!(state.error.is_some(), "invalid form shows an error");
    state.set_homeserver_url("https://matrix.example.org".to_string());
    state.set_access_token("tok".to_string());
    assert_eq!(handle_login_key(&mut state, key(KeyCode::Enter)), Command::SubmitLogin);
}

#[test]
fn q_quits() {
    let mut state = LoginState::default();
    assert_eq!(handle_login_key(&mut state, key(KeyCode::Char('q'))), Command::Quit);
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p tui typing_goes_to_the_focused_field`
Expected: FAIL — the stub handler only maps `q`.

- [ ] **Step 3: Write the implementation — full `crates/tui/src/screens/login.rs`**

```rust
use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use state::screens::LoginState;

pub fn handle_login_key(state: &mut LoginState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Quit,
        KeyCode::Char('\t') => {
            state.toggle_focus();
            Command::None
        }
        KeyCode::Char(c) => {
            state.insert_char(c);
            Command::None
        }
        KeyCode::Backspace => {
            state.backspace();
            Command::None
        }
        KeyCode::Enter => match state.validation_error() {
            None => Command::SubmitLogin,
            Some(error) => {
                state.error = Some(error);
                Command::None
            }
        },
        _ => Command::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use state::screens::LoginField;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn typing_goes_to_the_focused_field() {
        let mut state = LoginState::default();
        assert_eq!(handle_login_key(&mut state, key(KeyCode::Char('h'))), Command::None);
        assert_eq!(state.homeserver_url, "h");
        assert_eq!(handle_login_key(&mut state, key(KeyCode::Char('\t'))), Command::None);
        assert_eq!(state.focus, LoginField::AccessToken);
        assert_eq!(handle_login_key(&mut state, key(KeyCode::Char('t'))), Command::None);
        assert_eq!(state.access_token, "t");
        assert_eq!(handle_login_key(&mut state, key(KeyCode::Backspace)), Command::None);
        assert_eq!(state.access_token, "");
    }

    #[test]
    fn enter_submits_only_when_valid() {
        let mut state = LoginState::default();
        assert_eq!(handle_login_key(&mut state, key(KeyCode::Enter)), Command::None);
        assert!(state.error.is_some(), "invalid form shows an error");
        state.set_homeserver_url("https://matrix.example.org".to_string());
        state.set_access_token("tok".to_string());
        assert_eq!(handle_login_key(&mut state, key(KeyCode::Enter)), Command::SubmitLogin);
    }

    #[test]
    fn q_quits() {
        let mut state = LoginState::default();
        assert_eq!(handle_login_key(&mut state, key(KeyCode::Char('q'))), Command::Quit);
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: `test result: ok. 21 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/screens/login.rs
git commit -m "feat(tui): login screen key handler"
```

### Task 8.2: Login screen — render

**Files:**
- Modify: `crates/tui/src/screens/login.rs` (replace the stub `render_login`)

- [ ] **Step 1: Write the failing test (append to the `tests` module)**

```rust
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn login_render_shows_fields_and_error() {
    let mut state = LoginState::default();
    state.set_homeserver_url("https://matrix.example.org".to_string());
    state.set_access_token("secret".to_string());
    state.error = Some("Invalid token".to_string());

    let backend = TestBackend::new(70, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            render_login(&state, frame, area);
        })
        .unwrap();
    let rendered: String = terminal.backend().buffer().content().iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    assert!(rendered.contains("Matrix Agent Workspace"), "{rendered}");
    assert!(rendered.contains("https://matrix.example.org"), "{rendered}");
    assert!(rendered.contains("Invalid token"), "{rendered}");
    // The token must never be rendered verbatim.
    assert!(!rendered.contains("secret"), "token is masked: {rendered}");
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p tui login_render_shows_fields_and_error`
Expected: FAIL — the stub renders nothing, so `rendered.contains("Matrix Agent Workspace")` fails.

- [ ] **Step 3: Write the implementation — replace the imports at the top of `crates/tui/src/screens/login.rs` and the stub `render_login` with the code below**

```rust
use ratatui::backend::Backend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use state::screens::LoginField;

fn masked_token(token: &str) -> String {
    if token.is_empty() {
        "(empty)".to_string()
    } else {
        "•".repeat(token.len().min(12))
    }
}

pub fn render_login<B: Backend>(state: &LoginState, frame: &mut Frame<B>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(area);

    let title = Paragraph::new("Matrix Agent Workspace — Sign in")
        .alignment(Alignment::Center)
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    let url_focus = state.focus == LoginField::HomeserverUrl;
    let url_border = if url_focus {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let url = Paragraph::new(state.homeserver_url.as_str())
        .block(Block::default().borders(Borders::ALL).title(if url_focus { "Homeserver URL *" } else { "Homeserver URL" }).border_style(url_border));
    frame.render_widget(url, chunks[1]);

    let token_focus = state.focus == LoginField::AccessToken;
    let token_border = if token_focus {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let token = Paragraph::new(masked_token(&state.access_token))
        .block(Block::default().borders(Borders::ALL).title(if token_focus { "Access token *" } else { "Access token" }).border_style(token_border));
    frame.render_widget(token, chunks[2]);

    let error = match &state.error {
        Some(message) => Paragraph::new(message.as_str()).style(Style::default().fg(Color::Red)),
        None => Paragraph::new(""),
    };
    frame.render_widget(error, chunks[3]);

    let hints = Paragraph::new("Tab: switch field   Enter: sign in   q: quit");
    frame.render_widget(hints, chunks[4]);
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: `test result: ok. 22 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/screens/login.rs
git commit -m "feat(tui): login screen render"
```

### Task 8.3: Workspaces screen — key handler

**Files:**
- Replace: `crates/tui/src/screens/workspaces.rs`

- [ ] **Step 1: Write the failing tests**

```rust
use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use state::screens::WorkspacesState;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn with_workspaces() -> WorkspacesState {
    let mut state = WorkspacesState::new();
    state.add_workspace(api_client::WorkspaceSelection {
        workspace_id: "ws_1".to_string(),
        name: "Alpha".to_string(),
        owner_id: "@u:example.org".to_string(),
        status: "active".to_string(),
        created_at: "2026-08-15T00:00:00.000Z".to_string(),
    });
    state
}

#[test]
fn workspaces_navigation_and_create_mode() {
    let mut state = with_workspaces();
    assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Char('j'))), Command::None);
    assert_eq!(state.selected(), 0);
    assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Enter)), Command::NavigateToRooms);
    assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Char('n'))), Command::None);
    assert!(state.creating);
    assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Char('o'))), Command::None);
    assert_eq!(state.name_input, "o");
    assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Backspace)), Command::None);
    assert_eq!(state.name_input, "");
    assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Char('p'))), Command::None);
    assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Enter)), Command::CreateWorkspace);
}

#[test]
fn workspaces_q_quits() {
    let mut state = with_workspaces();
    assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Char('q'))), Command::Quit);
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p tui workspaces_navigation_and_create_mode`
Expected: FAIL — the stub only maps `q`.

- [ ] **Step 3: Write the implementation — full `crates/tui/src/screens/workspaces.rs`**

```rust
use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use state::screens::WorkspacesState;

pub fn handle_workspaces_key(state: &mut WorkspacesState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Quit,
        KeyCode::Char('n') => {
            state.creating = !state.creating;
            Command::None
        }
        KeyCode::Char(c) if state.creating => {
            let mut value = state.name_input.clone();
            value.push(c);
            state.set_name_input(value);
            Command::None
        }
        KeyCode::Backspace if state.creating => {
            let mut value = state.name_input.clone();
            value.pop();
            state.set_name_input(value);
            Command::None
        }
        KeyCode::Enter if state.creating => Command::CreateWorkspace,
        KeyCode::Char('j') | KeyCode::Down => {
            state.select_next();
            Command::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.select_prev();
            Command::None
        }
        KeyCode::Enter => {
            if state.selected().is_some() {
                Command::NavigateToRooms
            } else {
                Command::None
            }
        }
        _ => Command::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn with_workspaces() -> WorkspacesState {
        let mut state = WorkspacesState::new();
        state.add_workspace(api_client::WorkspaceSelection {
            workspace_id: "ws_1".to_string(),
            name: "Alpha".to_string(),
            owner_id: "@u:example.org".to_string(),
            status: "active".to_string(),
            created_at: "2026-08-15T00:00:00.000Z".to_string(),
        });
        state
    }

    #[test]
    fn workspaces_navigation_and_create_mode() {
        let mut state = with_workspaces();
        assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Char('j'))), Command::None);
        assert_eq!(state.selected(), 0);
        assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Enter)), Command::NavigateToRooms);
        assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Char('n'))), Command::None);
        assert!(state.creating);
        assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Char('o'))), Command::None);
        assert_eq!(state.name_input, "o");
        assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Backspace)), Command::None);
        assert_eq!(state.name_input, "");
        assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Char('p'))), Command::None);
        assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Enter)), Command::CreateWorkspace);
    }

    #[test]
    fn workspaces_q_quits() {
        let mut state = with_workspaces();
        assert_eq!(handle_workspaces_key(&mut state, key(KeyCode::Char('q'))), Command::Quit);
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: `test result: ok. 24 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/screens/workspaces.rs
git commit -m "feat(tui): workspaces screen key handler"
```

### Task 8.4: Workspaces screen — render

**Files:**
- Modify: `crates/tui/src/screens/workspaces.rs` (replace the stub `render_workspaces`)

- [ ] **Step 1: Write the failing test (append to the `tests` module)**

```rust
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn workspaces_render_lists_workspaces_and_create_input() {
    let mut state = with_workspaces();
    state.creating = true;
    state.set_name_input("ops".to_string());

    let backend = TestBackend::new(70, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            render_workspaces(&state, frame, area);
        })
        .unwrap();
    let rendered: String = terminal.backend().buffer().content().iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    assert!(rendered.contains("Alpha"), "{rendered}");
    assert!(rendered.contains("ops"), "{rendered}");
    assert!(rendered.contains("Workspaces"), "{rendered}");
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p tui workspaces_render_lists_workspaces_and_create_input`
Expected: FAIL — the stub renders nothing.

- [ ] **Step 3: Write the implementation — replace the imports at the top of `crates/tui/src/screens/workspaces.rs` and the stub `render_workspaces` with the code below**

```rust
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

pub fn render_workspaces<B: Backend>(state: &WorkspacesState, frame: &mut Frame<B>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(area);

    let title = Paragraph::new("Workspaces")
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = state
        .workspaces
        .iter()
        .enumerate()
        .map(|(index, workspace)| {
            let marker = if index == state.selected { ">" } else { " " };
            ListItem::new(format!(
                "{marker} {:<24} {}",
                workspace.name,
                workspace.status
            ))
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Known workspaces"));
    frame.render_widget(list, chunks[1]);

    let create = if state.creating {
        Paragraph::new(state.name_input.as_str())
            .block(Block::default().borders(Borders::ALL).title("New workspace name (Enter to create)").border_style(Style::default().fg(Color::Cyan)))
    } else {
        Paragraph::new("Press n to create a workspace")
    };
    frame.render_widget(create, chunks[2]);

    let error = match &state.error {
        Some(message) => Paragraph::new(message.as_str()).style(Style::default().fg(Color::Red)),
        None => Paragraph::new(""),
    };
    frame.render_widget(error, chunks[3]);

    let hints = Paragraph::new("j/k: move   Enter: open   n: new workspace   q: quit");
    frame.render_widget(hints, chunks[4]);
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: `test result: ok. 25 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/screens/workspaces.rs
git commit -m "feat(tui): workspaces screen render"
```

### Task 8.5: Rooms + RoomBinding screens — key handlers

**Files:**
- Replace: `crates/tui/src/screens/rooms.rs`

- [ ] **Step 1: Write the failing tests**

```rust
use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use state::screens::{RoomBindingState, RoomsState};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn room(room_id: &str, workspace_id: Option<&str>) -> api_client::RoomSummary {
    api_client::RoomSummary {
        room_id: room_id.to_string(),
        homeserver_url: "https://example.org".to_string(),
        display_name: Some(room_id.to_string()),
        workspace_id: workspace_id.map(|value| value.to_string()),
    }
}

#[test]
fn rooms_enter_opens_composer_when_bound_else_binding() {
    let mut bound = RoomsState::new("ws_1".to_string());
    bound.set_rooms(vec![room("!a:example.org", Some("ws_1"))]);
    assert_eq!(handle_rooms_key(&mut bound, key(KeyCode::Enter)), Command::NavigateToComposer);

    let mut unbound = RoomsState::new("ws_1".to_string());
    unbound.set_rooms(vec![room("!a:example.org", None)]);
    assert_eq!(handle_rooms_key(&mut unbound, key(KeyCode::Enter)), Command::NavigateToRoomBinding);
}

#[test]
fn rooms_refresh_and_back() {
    let mut state = RoomsState::new("ws_1".to_string());
    assert_eq!(handle_rooms_key(&mut state, key(KeyCode::Char('r'))), Command::RefreshRooms);
    assert_eq!(handle_rooms_key(&mut state, key(KeyCode::Char('q'))), Command::Back);
}

#[test]
fn room_binding_enter_confirms_bind_and_q_goes_back() {
    let mut state = RoomBindingState::new(room("!a:example.org", None), "ws_1".to_string());
    assert_eq!(handle_room_binding_key(&mut state, key(KeyCode::Char('y'))), Command::BindRoom);
    assert_eq!(handle_room_binding_key(&mut state, key(KeyCode::Char('q'))), Command::Back);
    let mut state = RoomBindingState::new(room("!a:example.org", None), "ws_1".to_string());
    assert_eq!(handle_room_binding_key(&mut state, key(KeyCode::Enter)), Command::BindRoom);
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p tui rooms_enter_opens_composer_when_bound_else_binding`
Expected: FAIL — the stub only maps `q`.

- [ ] **Step 3: Write the implementation — full `crates/tui/src/screens/rooms.rs`**

```rust
use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use state::screens::{RoomBindingState, RoomsState};

pub fn handle_rooms_key(state: &mut RoomsState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        KeyCode::Char('r') => Command::RefreshRooms,
        KeyCode::Char('j') | KeyCode::Down => {
            state.select_next();
            Command::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.select_prev();
            Command::None
        }
        KeyCode::Enter => {
            if state.selected_room().is_none() {
                Command::None
            } else if state.room_is_bound_to_workspace() {
                Command::NavigateToComposer
            } else {
                Command::NavigateToRoomBinding
            }
        }
        _ => Command::None,
    }
}

pub fn handle_room_binding_key(state: &mut RoomBindingState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Command::Back,
        KeyCode::Char('y') | KeyCode::Enter => Command::BindRoom,
        _ => Command::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn room(room_id: &str, workspace_id: Option<&str>) -> api_client::RoomSummary {
        api_client::RoomSummary {
            room_id: room_id.to_string(),
            homeserver_url: "https://example.org".to_string(),
            display_name: Some(room_id.to_string()),
            workspace_id: workspace_id.map(|value| value.to_string()),
        }
    }

    #[test]
    fn rooms_enter_opens_composer_when_bound_else_binding() {
        let mut bound = RoomsState::new("ws_1".to_string());
        bound.set_rooms(vec![room("!a:example.org", Some("ws_1"))]);
        assert_eq!(handle_rooms_key(&mut bound, key(KeyCode::Enter)), Command::NavigateToComposer);

        let mut unbound = RoomsState::new("ws_1".to_string());
        unbound.set_rooms(vec![room("!a:example.org", None)]);
        assert_eq!(handle_rooms_key(&mut unbound, key(KeyCode::Enter)), Command::NavigateToRoomBinding);
    }

    #[test]
    fn rooms_refresh_and_back() {
        let mut state = RoomsState::new("ws_1".to_string());
        assert_eq!(handle_rooms_key(&mut state, key(KeyCode::Char('r'))), Command::RefreshRooms);
        assert_eq!(handle_rooms_key(&mut state, key(KeyCode::Char('q'))), Command::Back);
    }

    #[test]
    fn room_binding_enter_confirms_bind_and_q_goes_back() {
        let mut state = RoomBindingState::new(room("!a:example.org", None), "ws_1".to_string());
        assert_eq!(handle_room_binding_key(&mut state, key(KeyCode::Char('y'))), Command::BindRoom);
        assert_eq!(handle_room_binding_key(&mut state, key(KeyCode::Char('q'))), Command::Back);
        let mut state = RoomBindingState::new(room("!a:example.org", None), "ws_1".to_string());
        assert_eq!(handle_room_binding_key(&mut state, key(KeyCode::Enter)), Command::BindRoom);
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: `test result: ok. 28 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/screens/rooms.rs
git commit -m "feat(tui): rooms and room binding key handlers"
```

### Task 8.6: Rooms + RoomBinding screens — render

**Files:**
- Modify: `crates/tui/src/screens/rooms.rs` (replace the stub renders)

- [ ] **Step 1: Write the failing tests (append to the `tests` module)**

```rust
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn rooms_render_shows_rooms_and_binding_state() {
    let mut state = RoomsState::new("ws_1".to_string());
    state.set_rooms(vec![room("!a:example.org", Some("ws_1")), room("!b:example.org", None)]);
    let backend = TestBackend::new(70, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            render_rooms(&state, frame, area);
        })
        .unwrap();
    let rendered: String = terminal.backend().buffer().content().iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    assert!(rendered.contains("!a:example.org"), "{rendered}");
    assert!(rendered.contains("!b:example.org"), "{rendered}");
    assert!(rendered.contains("bound"), "{rendered}");
}

#[test]
fn room_binding_render_shows_room_and_workspace() {
    let state = RoomBindingState::new(room("!a:example.org", None), "ws_1".to_string());
    let backend = TestBackend::new(70, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            render_room_binding(&state, frame, area);
        })
        .unwrap();
    let rendered: String = terminal.backend().buffer().content().iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    assert!(rendered.contains("!a:example.org"), "{rendered}");
    assert!(rendered.contains("ws_1"), "{rendered}");
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p tui rooms_render_shows_rooms_and_binding_state`
Expected: FAIL — the stubs render nothing.

- [ ] **Step 3: Write the implementation — replace the imports at the top of `crates/tui/src/screens/rooms.rs` and the stub renders with the code below**

```rust
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

pub fn render_rooms<B: Backend>(state: &RoomsState, frame: &mut Frame<B>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5), Constraint::Length(2), Constraint::Length(1)])
        .split(area);

    let title = Paragraph::new(format!("Rooms — workspace {}", state.workspace_id))
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = state
        .rooms
        .iter()
        .enumerate()
        .map(|(index, room)| {
            let marker = if index == state.selected { ">" } else { " " };
            let binding = match room.workspace_id.as_deref() {
                Some(workspace_id) if workspace_id == state.workspace_id => "bound to this workspace",
                Some(_) => "bound elsewhere",
                None => "unbound",
            };
            ListItem::new(format!(
                "{marker} {:<40} {binding}",
                room.display_name.as_deref().unwrap_or(&room.room_id)
            ))
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Rooms"));
    frame.render_widget(list, chunks[1]);

    let error = match &state.error {
        Some(message) => Paragraph::new(message.as_str()).style(Style::default().fg(Color::Red)),
        None => Paragraph::new(""),
    };
    frame.render_widget(error, chunks[2]);

    let hints = Paragraph::new("j/k: move   Enter: compose (bound) or bind   r: refresh   q: back");
    frame.render_widget(hints, chunks[3]);
}

pub fn render_room_binding<B: Backend>(state: &RoomBindingState, frame: &mut Frame<B>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(2), Constraint::Length(2), Constraint::Length(1)])
        .split(area);

    let title = Paragraph::new("Bind room to workspace").style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);
    frame.render_widget(Paragraph::new(format!("Room:      {}", state.room.room_id)), chunks[1]);
    frame.render_widget(Paragraph::new(format!("Workspace: {}", state.workspace_id)), chunks[2]);

    let error = match &state.error {
        Some(message) => Paragraph::new(message.as_str()).style(Style::default().fg(Color::Red)),
        None => Paragraph::new(""),
    };
    frame.render_widget(error, chunks[3]);
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: `test result: ok. 30 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/screens/rooms.rs
git commit -m "feat(tui): rooms and room binding renders"
```

### Task 8.7: RunComposer screen — key handler

**Files:**
- Replace: `crates/tui/src/screens/run_composer.rs`

- [ ] **Step 1: Write the failing tests**

```rust
use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use state::screens::{RunComposerState, SPECIALISTS};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn composer() -> RunComposerState {
    RunComposerState::new("!a:example.org".to_string(), "ws_1".to_string())
}

#[test]
fn composer_types_prompt_and_toggles_mode_and_specialists() {
    let mut state = composer();
    assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('h'))), Command::None);
    assert_eq!(state.prompt, "h");
    assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('p'))), Command::None);
    assert_eq!(state.mode, Some(api_client::RunMode::Parallel));
    assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('s'))), Command::None);
    assert_eq!(state.mode, Some(api_client::RunMode::Sequential));
    assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char(' '))), Command::None);
    assert_eq!(state.selected_specialists, vec![SPECIALISTS[0].0]);
    assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Backspace)), Command::None);
    assert_eq!(state.prompt, "");
}

#[test]
fn composer_enter_launches_when_valid() {
    let mut state = composer();
    assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Enter)), Command::None, "invalid form");
    state.set_prompt("Go".to_string());
    state.toggle_mode(api_client::RunMode::Parallel);
    state.toggle_specialist("repo-reader");
    assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Enter)), Command::LaunchRun);
}

#[test]
fn composer_cursor_moves_with_jk() {
    let mut state = composer();
    assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('j'))), Command::None);
    assert_eq!(state.specialist_cursor, 1);
    assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('k'))), Command::None);
    assert_eq!(state.specialist_cursor, 0);
    assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('q'))), Command::Back);
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p tui composer_types_prompt_and_toggles_mode_and_specialists`
Expected: FAIL — the stub only maps `q`.

- [ ] **Step 3: Write the implementation — full `crates/tui/src/screens/run_composer.rs`**

```rust
use crate::app::Command;
use api_client::RunMode;
use crossterm::event::{KeyCode, KeyEvent};
use state::screens::{RunComposerState, SPECIALISTS};

pub fn handle_run_composer_key(state: &mut RunComposerState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        KeyCode::Char('p') => {
            state.toggle_mode(RunMode::Parallel);
            Command::None
        }
        KeyCode::Char('s') => {
            state.toggle_mode(RunMode::Sequential);
            Command::None
        }
        KeyCode::Char(' ') => {
            state.toggle_specialist_at_cursor();
            Command::None
        }
        KeyCode::Char('j') | KeyCode::Down => {
            state.move_specialist_cursor_next();
            Command::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.move_specialist_cursor_prev();
            Command::None
        }
        KeyCode::Char(c) => {
            let mut value = state.prompt.clone();
            value.push(c);
            state.set_prompt(value);
            Command::None
        }
        KeyCode::Backspace => {
            let mut value = state.prompt.clone();
            value.pop();
            state.set_prompt(value);
            Command::None
        }
        KeyCode::Enter => match state.validation_error() {
            None => Command::LaunchRun,
            Some(error) => {
                state.error = Some(error);
                Command::None
            }
        },
        _ => Command::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn composer() -> RunComposerState {
        RunComposerState::new("!a:example.org".to_string(), "ws_1".to_string())
    }

    #[test]
    fn composer_types_prompt_and_toggles_mode_and_specialists() {
        let mut state = composer();
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('h'))), Command::None);
        assert_eq!(state.prompt, "h");
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('p'))), Command::None);
        assert_eq!(state.mode, Some(RunMode::Parallel));
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('s'))), Command::None);
        assert_eq!(state.mode, Some(RunMode::Sequential));
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char(' '))), Command::None);
        assert_eq!(state.selected_specialists, vec![SPECIALISTS[0].0]);
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Backspace)), Command::None);
        assert_eq!(state.prompt, "");
    }

    #[test]
    fn composer_enter_launches_when_valid() {
        let mut state = composer();
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Enter)), Command::None, "invalid form");
        state.set_prompt("Go".to_string());
        state.toggle_mode(RunMode::Parallel);
        state.toggle_specialist("repo-reader");
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Enter)), Command::LaunchRun);
    }

    #[test]
    fn composer_cursor_moves_with_jk() {
        let mut state = composer();
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('j'))), Command::None);
        assert_eq!(state.specialist_cursor, 1);
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('k'))), Command::None);
        assert_eq!(state.specialist_cursor, 0);
        assert_eq!(handle_run_composer_key(&mut state, key(KeyCode::Char('q'))), Command::Back);
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: `test result: ok. 33 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/screens/run_composer.rs
git commit -m "feat(tui): run composer key handler"
```

### Task 8.8: RunComposer screen — render

**Files:**
- Modify: `crates/tui/src/screens/run_composer.rs` (replace the stub `render_run_composer`)

- [ ] **Step 1: Write the failing test (append to the `tests` module)**

```rust
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn composer_render_shows_prompt_mode_and_specialists() {
    let mut state = composer();
    state.set_prompt("Summarize the PRs".to_string());
    state.toggle_mode(RunMode::Parallel);
    state.toggle_specialist("pr-reader");

    let backend = TestBackend::new(80, 16);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            render_run_composer(&state, frame, area);
        })
        .unwrap();
    let rendered: String = terminal.backend().buffer().content().iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    assert!(rendered.contains("Summarize the PRs"), "{rendered}");
    assert!(rendered.contains("parallel"), "{rendered}");
    assert!(rendered.contains("Pull Request reader"), "{rendered}");
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p tui composer_render_shows_prompt_mode_and_specialists`
Expected: FAIL — the stub renders nothing.

- [ ] **Step 3: Write the implementation — replace the imports at the top of `crates/tui/src/screens/run_composer.rs` and the stub `render_run_composer` with the code below**

```rust
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

pub fn render_run_composer<B: Backend>(state: &RunComposerState, frame: &mut Frame<B>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Min(4),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(area);

    let title = Paragraph::new(format!("Compose run — room {}", state.room_id))
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    let prompt = Paragraph::new(state.prompt.as_str())
        .block(Block::default().borders(Borders::ALL).title("Prompt (type to edit)").border_style(Style::default().fg(Color::Cyan)));
    frame.render_widget(prompt, chunks[1]);

    let mode = match state.mode {
        Some(RunMode::Parallel) => "parallel (p)",
        Some(RunMode::Sequential) => "sequential (s)",
        None => "unset — press p or s",
    };
    frame.render_widget(Paragraph::new(format!("Mode: {mode}")), chunks[2]);

    let items: Vec<ListItem> = SPECIALISTS
        .iter()
        .enumerate()
        .map(|(index, (id, name))| {
            let marker = if index == state.specialist_cursor { ">" } else { " " };
            let selected = if state.selected_specialists.iter().any(|value| value == id) { "[x]" } else { "[ ]" };
            ListItem::new(format!("{marker} {selected} {name}"))
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Specialists (space to toggle)"));
    frame.render_widget(list, chunks[3]);

    let error = match &state.error {
        Some(message) => Paragraph::new(message.as_str()).style(Style::default().fg(Color::Red)),
        None => Paragraph::new(""),
    };
    frame.render_widget(error, chunks[4]);

    let hints = Paragraph::new("type: prompt   p/s: mode   j/k+space: specialists   Enter: launch   q: back");
    frame.render_widget(hints, chunks[5]);
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: `test result: ok. 34 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/screens/run_composer.rs
git commit -m "feat(tui): run composer render"
```

### Task 8.9: Run screen — key handler

**Files:**
- Replace: `crates/tui/src/screens/run.rs`

- [ ] **Step 1: Write the failing tests**

```rust
use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use state::screens::RunState;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn run_keys_map_to_commands() {
    let mut state = RunState::new("r1".to_string(), "ws_1".to_string());
    assert_eq!(handle_run_key(&mut state, key(KeyCode::Char('c'))), Command::CancelRun);
    assert_eq!(handle_run_key(&mut state, key(KeyCode::Char('r'))), Command::RefreshDeliveries);
    assert_eq!(handle_run_key(&mut state, key(KeyCode::Char('g'))), Command::NavigateToGitHubWorkspace);
    assert_eq!(handle_run_key(&mut state, key(KeyCode::Char('q'))), Command::Back);
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p tui run_keys_map_to_commands`
Expected: FAIL — the stub only maps `q`.

- [ ] **Step 3: Write the implementation — full `crates/tui/src/screens/run.rs`**

```rust
use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use state::screens::RunState;

pub fn handle_run_key(state: &mut RunState, key: KeyEvent) -> Command {
    match key.code {
        KeyCode::Char('q') => Command::Back,
        KeyCode::Char('c') => Command::CancelRun,
        KeyCode::Char('r') => Command::RefreshDeliveries,
        KeyCode::Char('g') => Command::NavigateToGitHubWorkspace,
        _ => Command::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn run_keys_map_to_commands() {
        let mut state = RunState::new("r1".to_string(), "ws_1".to_string());
        assert_eq!(handle_run_key(&mut state, key(KeyCode::Char('c'))), Command::CancelRun);
        assert_eq!(handle_run_key(&mut state, key(KeyCode::Char('r'))), Command::RefreshDeliveries);
        assert_eq!(handle_run_key(&mut state, key(KeyCode::Char('g'))), Command::NavigateToGitHubWorkspace);
        assert_eq!(handle_run_key(&mut state, key(KeyCode::Char('q'))), Command::Back);
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: `test result: ok. 35 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/screens/run.rs
git commit -m "feat(tui): run screen key handler"
```

### Task 8.10: Run screen — render (live timeline + deliveries + terminal)

**Files:**
- Modify: `crates/tui/src/screens/run.rs` (replace the stub `render_run`)

- [ ] **Step 1: Write the failing tests (append to the `tests` module)**

```rust
use api_client::{EventVisibility, MatrixDelivery, MatrixDeliveryStatus, RunEvent, RunEventType};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn run_event(sequence: u64, event_type: RunEventType) -> RunEvent {
    RunEvent {
        id: format!("ev_{sequence}"),
        run_id: "r1".to_string(),
        sequence,
        event_type,
        version: 1,
        occurred_at: "2026-08-15T00:00:00.000Z".to_string(),
        visibility: EventVisibility::RoomAndOwner,
        payload: serde_json::json!({ "note": format!("step {sequence}") }),
    }
}

#[test]
fn run_render_shows_timeline_deliveries_and_reconnect_banner() {
    let mut state = RunState::new("r1".to_string(), "ws_1".to_string());
    state.accept(run_event(1, RunEventType::RunStarted));
    state.accept(run_event(2, RunEventType::SpecialistProgress));
    state.set_deliveries(vec![MatrixDelivery {
        sequence: 1,
        status: MatrixDeliveryStatus::Delivered,
    }]);
    state.set_reconnecting(true);

    let backend = TestBackend::new(90, 18);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            render_run(&state, frame, area);
        })
        .unwrap();
    let rendered: String = terminal.backend().buffer().content().iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    assert!(rendered.contains("Run r1"), "{rendered}");
    assert!(rendered.contains("specialist.progress"), "{rendered}");
    assert!(rendered.contains("delivered"), "{rendered}");
    assert!(rendered.contains("reconnecting"), "{rendered}");
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p tui run_render_shows_timeline_deliveries_and_reconnect_banner`
Expected: FAIL — the stub renders nothing.

- [ ] **Step 3: Write the implementation — replace the imports at the top of `crates/tui/src/screens/run.rs` and the stub `render_run` with the code below**

```rust
use api_client::MatrixDeliveryStatus;
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

fn delivery_label(status: MatrixDeliveryStatus) -> &'static str {
    match status {
        MatrixDeliveryStatus::Pending => "pending",
        MatrixDeliveryStatus::Delivered => "delivered",
        MatrixDeliveryStatus::Failed => "failed",
        MatrixDeliveryStatus::Dead => "dead",
    }
}

pub fn render_run<B: Backend>(state: &RunState, frame: &mut Frame<B>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Min(6),
            Constraint::Length(5),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(area);

    let terminal_line = if state.is_terminal() {
        "Run finished (terminal state)".to_string()
    } else if state.cancel_requested {
        "Cancellation requested".to_string()
    } else {
        "Running".to_string()
    };
    let title = Paragraph::new(format!("Run {} — {terminal_line}", state.run_id))
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    let reconnect = if state.reconnecting {
        Paragraph::new("reconnecting…").style(Style::default().fg(Color::Yellow))
    } else {
        Paragraph::new("")
    };
    frame.render_widget(reconnect, chunks[1]);

    let items: Vec<ListItem> = state
        .events()
        .iter()
        .map(|event| ListItem::new(format!("#{} {} {}", event.sequence, event.event_type.as_str(), event.payload)))
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Timeline"));
    frame.render_widget(list, chunks[2]);

    let deliveries: Vec<ListItem> = state
        .deliveries
        .iter()
        .map(|delivery| ListItem::new(format!("sequence {}: {}", delivery.sequence, delivery_label(delivery.status))))
        .collect();
    let deliveries = List::new(deliveries).block(Block::default().borders(Borders::ALL).title("Matrix delivery (authoritative)"));
    frame.render_widget(deliveries, chunks[3]);

    let error = match &state.error {
        Some(message) => Paragraph::new(message.as_str()).style(Style::default().fg(Color::Red)),
        None => Paragraph::new(""),
    };
    frame.render_widget(error, chunks[4]);

    let hints = Paragraph::new("c: cancel   r: refresh deliveries   g: GitHub workspace   q: back");
    frame.render_widget(hints, chunks[5]);
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: `test result: ok. 36 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/screens/run.rs
git commit -m "feat(tui): run screen render with timeline and deliveries"
```

### Task 8.11: GitHubWorkspace screen — key handler

**Files:**
- Replace: `crates/tui/src/screens/github.rs`

- [ ] **Step 1: Write the failing tests**

```rust
use crate::app::Command;
use api_client::GithubRepositorySummary;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use state::screens::{GithubPanel, GitHubWorkspaceState};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn repo() -> GithubRepositorySummary {
    GithubRepositorySummary {
        id: 1,
        name: "repo".to_string(),
        full_name: "octo/repo".to_string(),
        owner: "octo".to_string(),
        private: false,
        default_branch: "main".to_string(),
        description: None,
        html_url: "https://github.com/octo/repo".to_string(),
        archived: false,
    }
}

fn github() -> GitHubWorkspaceState {
    let mut state = GitHubWorkspaceState::new("ws_1".to_string(), "r1".to_string(), Some("inst_9".to_string()));
    state.set_repositories(vec![repo()]);
    state
}

#[test]
fn github_panel_switching_and_navigation() {
    let mut state = github();
    assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('2'))), Command::None);
    assert_eq!(state.panel, GithubPanel::Issues);
    assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('4'))), Command::None);
    assert_eq!(state.panel, GithubPanel::Audit);
    assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('q'))), Command::Back);
    assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('r'))), Command::RefreshPanel);
}

#[test]
fn github_grant_and_mutation_flow() {
    let mut state = github();
    assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('g'))), Command::RequestGrant);
    assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('m'))), Command::None);
    assert!(state.mutation_mode);
    assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('t'))), Command::None);
    assert_eq!(state.mutation_title, "t");
    assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Enter)), Command::None);
    assert!(state.confirmation.is_some(), "confirmation draft shown");
}

#[test]
fn github_confirmation_keys_confirm_or_dismiss() {
    let mut state = github();
    state.confirmation = state.begin_mutation("Test issue".to_string());
    assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('y'))), Command::ConfirmMutation);
    let mut state = github();
    state.confirmation = state.begin_mutation("Test issue".to_string());
    assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('n'))), Command::None);
    assert!(state.confirmation.is_none(), "dismissed");
}
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p tui github_grant_and_mutation_flow`
Expected: FAIL — the stub only maps `q`.

- [ ] **Step 3: Write the implementation — full `crates/tui/src/screens/github.rs`**

```rust
use crate::app::Command;
use crossterm::event::{KeyCode, KeyEvent};
use state::screens::{GithubPanel, GitHubWorkspaceState};

pub fn handle_github_workspace_key(state: &mut GitHubWorkspaceState, key: KeyEvent) -> Command {
    if state.confirmation.is_some() {
        return match key.code {
            KeyCode::Char('y') => Command::ConfirmMutation,
            KeyCode::Char('n') | KeyCode::Char('q') => {
                state.confirmation = None;
                Command::None
            }
            _ => Command::None,
        };
    }
    match key.code {
        KeyCode::Char('q') => Command::Back,
        KeyCode::Char('1') => {
            state.switch_panel(GithubPanel::Repositories);
            Command::None
        }
        KeyCode::Char('2') => {
            state.switch_panel(GithubPanel::Issues);
            Command::None
        }
        KeyCode::Char('3') => {
            state.switch_panel(GithubPanel::PullRequests);
            Command::None
        }
        KeyCode::Char('4') => {
            state.switch_panel(GithubPanel::Audit);
            Command::None
        }
        KeyCode::Char('j') | KeyCode::Down => {
            state.select_next();
            Command::None
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.select_prev();
            Command::None
        }
        KeyCode::Char('r') => Command::RefreshPanel,
        KeyCode::Char('g') => Command::RequestGrant,
        KeyCode::Char('m') => {
            state.mutation_mode = !state.mutation_mode;
            Command::None
        }
        KeyCode::Char(c) if state.mutation_mode => {
            let mut value = state.mutation_title.clone();
            value.push(c);
            state.set_mutation_title(value);
            Command::None
        }
        KeyCode::Backspace if state.mutation_mode => {
            let mut value = state.mutation_title.clone();
            value.pop();
            state.set_mutation_title(value);
            Command::None
        }
        KeyCode::Enter if state.mutation_mode => {
            let title = state.mutation_title.clone();
            match state.begin_mutation(title) {
                Some(draft) => {
                    state.confirmation = Some(draft);
                    state.mutation_mode = false;
                    Command::None
                }
                None => {
                    state.error = Some("Provide an issue title before confirming".to_string());
                    Command::None
                }
            }
        }
        _ => Command::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use api_client::GithubRepositorySummary;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn repo() -> GithubRepositorySummary {
        GithubRepositorySummary {
            id: 1,
            name: "repo".to_string(),
            full_name: "octo/repo".to_string(),
            owner: "octo".to_string(),
            private: false,
            default_branch: "main".to_string(),
            description: None,
            html_url: "https://github.com/octo/repo".to_string(),
            archived: false,
        }
    }

    fn github() -> GitHubWorkspaceState {
        let mut state = GitHubWorkspaceState::new("ws_1".to_string(), "r1".to_string(), Some("inst_9".to_string()));
        state.set_repositories(vec![repo()]);
        state
    }

    #[test]
    fn github_panel_switching_and_navigation() {
        let mut state = github();
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('2'))), Command::None);
        assert_eq!(state.panel, GithubPanel::Issues);
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('4'))), Command::None);
        assert_eq!(state.panel, GithubPanel::Audit);
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('q'))), Command::Back);
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('r'))), Command::RefreshPanel);
    }

    #[test]
    fn github_grant_and_mutation_flow() {
        let mut state = github();
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('g'))), Command::RequestGrant);
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('m'))), Command::None);
        assert!(state.mutation_mode);
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('t'))), Command::None);
        assert_eq!(state.mutation_title, "t");
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Enter)), Command::None);
        assert!(state.confirmation.is_some(), "confirmation draft shown");
    }

    #[test]
    fn github_confirmation_keys_confirm_or_dismiss() {
        let mut state = github();
        state.confirmation = state.begin_mutation("Test issue".to_string());
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('y'))), Command::ConfirmMutation);
        let mut state = github();
        state.confirmation = state.begin_mutation("Test issue".to_string());
        assert_eq!(handle_github_workspace_key(&mut state, key(KeyCode::Char('n'))), Command::None);
        assert!(state.confirmation.is_none(), "dismissed");
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: `test result: ok. 39 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/screens/github.rs
git commit -m "feat(tui): github workspace key handler"
```

### Task 8.12: GitHubWorkspace screen — render (panels + confirmation)

**Files:**
- Modify: `crates/tui/src/screens/github.rs` (replace the stub `render_github_workspace`)

- [ ] **Step 1: Write the failing tests (append to the `tests` module)**

```rust
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn github_render_shows_panels_and_confirmation() {
    let mut state = github();
    state.switch_panel(GithubPanel::Repositories);
    state.confirmation = state.begin_mutation("Test issue".to_string());

    let backend = TestBackend::new(90, 18);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            render_github_workspace(&state, frame, area);
        })
        .unwrap();
    let rendered: String = terminal.backend().buffer().content().iter()
        .map(|cell| cell.symbol().to_string())
        .collect();
    assert!(rendered.contains("octo/repo"), "{rendered}");
    assert!(rendered.contains("Confirm mutation"), "{rendered}");
    assert!(rendered.contains("create_issue"), "{rendered}");
    assert!(rendered.contains("Test issue"), "{rendered}");
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p tui github_render_shows_panels_and_confirmation`
Expected: FAIL — the stub renders nothing.

- [ ] **Step 3: Write the implementation — replace the imports at the top of `crates/tui/src/screens/github.rs` and the stub `render_github_workspace` with the code below**

```rust
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use state::screens::{GithubPanel, MutationFlowStatus};

fn panel_title(panel: GithubPanel) -> &'static str {
    match panel {
        GithubPanel::Repositories => "Repositories (1)",
        GithubPanel::Issues => "Issues (2)",
        GithubPanel::PullRequests => "Pull requests (3)",
        GithubPanel::Audit => "Audit (4)",
    }
}

fn panel_items(state: &GitHubWorkspaceState) -> Vec<String> {
    match state.panel {
        GithubPanel::Repositories => state
            .repositories
            .iter()
            .map(|repo| format!("{} ({}branch {})", repo.full_name, if repo.private { "private " } else { "" }, repo.default_branch))
            .collect(),
        GithubPanel::Issues => state
            .issues
            .iter()
            .map(|issue| format!("#{} {} [{}]", issue.number, issue.title, issue.state))
            .collect(),
        GithubPanel::PullRequests => state
            .pulls
            .iter()
            .map(|pull| format!("#{} {} [{}]{}", pull.number, pull.title, pull.state, if pull.draft { " draft" } else { "" }))
            .collect(),
        GithubPanel::Audit => state
            .audit
            .iter()
            .map(|record| format!("{} {} {}", record.created_at, record.operation.as_deref().unwrap_or("-"), record.outcome))
            .collect(),
    }
}

fn mutation_status_line(status: MutationFlowStatus, command_id: Option<&String>) -> String {
    match status {
        MutationFlowStatus::Idle => String::new(),
        MutationFlowStatus::Submitting => "Submitting the approved command…".to_string(),
        MutationFlowStatus::Submitted => format!("Mutation queued. Command {}.", command_id.map(String::as_str).unwrap_or("-")),
        MutationFlowStatus::Succeeded => format!("Mutation completed. Command {}.", command_id.map(String::as_str).unwrap_or("-")),
        MutationFlowStatus::Denied => "Mutation denied. The write grant is missing or the approval does not match this exact command.".to_string(),
        MutationFlowStatus::Expired => "The approval expired. Confirm again to record a fresh approval.".to_string(),
        MutationFlowStatus::Failed => "The mutation failed. Review the audit history before retrying.".to_string(),
        MutationFlowStatus::Duplicate => format!("This exact command was already submitted; showing the recorded result. Command {}.", command_id.map(String::as_str).unwrap_or("-")),
    }
}

pub fn render_github_workspace<B: Backend>(state: &GitHubWorkspaceState, frame: &mut Frame<B>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(6),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(area);

    let title = Paragraph::new("GitHub workspace").style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);
    frame.render_widget(Paragraph::new(panel_title(state.panel)), chunks[1]);

    let items: Vec<ListItem> = panel_items(state)
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let marker = if index == state.selected_index { ">" } else { " " };
            ListItem::new(format!("{marker} {line}"))
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL));
    frame.render_widget(list, chunks[2]);

    let status_line = mutation_status_line(state.mutation_status, state.command_id.as_ref());
    let status = if status_line.is_empty() {
        match &state.error {
            Some(message) => Paragraph::new(message.as_str()).style(Style::default().fg(Color::Red)),
            None => Paragraph::new(""),
        }
    } else {
        Paragraph::new(status_line)
    };
    frame.render_widget(status, chunks[3]);

    if let Some(draft) = &state.confirmation {
        let operation_name = match draft.operation {
            api_client::GithubMutationOperation::CreateIssue => "create_issue",
            api_client::GithubMutationOperation::UpdateIssue => "update_issue",
            api_client::GithubMutationOperation::CommentIssue => "comment_issue",
            api_client::GithubMutationOperation::CreatePrComment => "create_pr_comment",
        };
        let confirmation = Paragraph::new(format!(
            "Confirm mutation — operation: {}   scope: {}   repository: {}\narguments: {}\nPress y to confirm, n to dismiss",
            operation_name,
            match draft.scope {
                api_client::GithubWriteScope::IssuesWrite => "issues:write",
                api_client::GithubWriteScope::PullRequestsWrite => "pull_requests:write",
            },
            draft.repository,
            draft.arguments,
        ))
        .block(Block::default().borders(Borders::ALL).title("Confirm mutation").border_style(Style::default().fg(Color::Yellow)));
        frame.render_widget(confirmation, chunks[4]);
    } else {
        let hints = if state.mutation_mode {
            Paragraph::new(format!("Issue title: {}   (Enter: review, q: cancel)", state.mutation_title))
        } else {
            Paragraph::new("1-4: panels   r: refresh   g: request write grant   m: compose mutation   q: back")
        };
        frame.render_widget(hints, chunks[4]);
    }

    let grant_line = match &state.grant {
        Some(grant) => format!("Grant {}{}", grant.grant_id, if grant.status == api_client::GrantStatus::Pending { " (pending approval)" } else { "" }),
        None => "No write grant requested yet".to_string(),
    };
    frame.render_widget(Paragraph::new(grant_line), chunks[5]);
}
```

The confirmation paragraph prints the operation by its wire name (the test asserts `rendered.contains("create_issue")`), not the Rust `Debug` spelling.

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p tui`
Expected: `test result: ok. 40 passed; 0 failed; ...`

- [ ] **Step 5: Commit**

```bash
git add crates/tui/src/screens/github.rs
git commit -m "feat(tui): github workspace render"
```

### Task 8.13: Full TUI suite gate

**Files:**
- none (verification only)

- [ ] **Step 1: Run the whole workspace**

Run: `cargo test --workspace`
Expected: api-client `52 passed`, state `25 passed`, tui `40 passed`; zero failures.

- [ ] **Step 2: Also run a release build (catches dead-code warnings treated as errors if any)**

Run: `cargo build --workspace --release`
Expected: `Finished \`release\` profile [optimized] target(s) in ...`

- [ ] **Step 3: Commit any drift**

```bash
git add -A
git commit -m "chore: group 8 verification" || echo "no changes to commit"
```

---

# Group 9: npm launcher package

`npm/matrix-workspace-tui` is a thin npm package: `postinstall` downloads the matching platform binary (linux-x64, linux-arm64, darwin-x64, darwin-arm64) from GitHub releases with a SHA-256 checksum check; the `bin` entry execs it. Release assets are named `matrix-workspace-tui-<platform>` plus `<...>.sha256`. The download base URL is overridable with `MATRIX_WORKSPACE_TUI_DOWNLOAD_BASE_URL` so tests run against a local static server.

### Task 9.1: Package skeleton + platform helper + launcher

**Files:**
- Create: `npm/matrix-workspace-tui/package.json`
- Create: `npm/matrix-workspace-tui/scripts/platform.js`
- Create: `npm/matrix-workspace-tui/index.js`

- [ ] **Step 1: Write `npm/matrix-workspace-tui/package.json`**

```json
{
  "name": "matrix-workspace-tui",
  "version": "0.1.0",
  "description": "Desktop TUI client for the Matrix Agent Workspace control plane",
  "license": "MIT",
  "bin": {
    "matrix-workspace-tui": "index.js"
  },
  "files": [
    "index.js",
    "scripts/"
  ],
  "scripts": {
    "postinstall": "node scripts/download.js",
    "test": "node --test test/"
  },
  "engines": {
    "node": ">=18"
  },
  "publishConfig": {
    "access": "public"
  }
}
```

- [ ] **Step 2: Write `npm/matrix-workspace-tui/scripts/platform.js`**

```js
'use strict';

/**
 * Map Node's platform/arch to the release asset platform name.
 * Asset names on GitHub releases: matrix-workspace-tui-<platform>.
 */
function getPlatform() {
  const mapping = {
    'linux-x64': 'linux-x64',
    'linux-arm64': 'linux-arm64',
    'darwin-x64': 'darwin-x64',
    'darwin-arm64': 'darwin-arm64',
  };
  const name = mapping[`${process.platform}-${process.arch}`];
  if (!name) {
    throw new Error(
      `Unsupported platform ${process.platform}-${process.arch}. ` +
        'Supported: linux-x64, linux-arm64, darwin-x64, darwin-arm64.',
    );
  }
  return { name, binaryName: `matrix-workspace-tui-${name}` };
}

module.exports = { getPlatform };
```

- [ ] **Step 3: Write `npm/matrix-workspace-tui/index.js`**

```js
#!/usr/bin/env node
'use strict';

const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');
const { getPlatform } = require('./scripts/platform');

const { binaryName } = getPlatform();
const binary = path.join(__dirname, 'bin', binaryName);

if (!fs.existsSync(binary)) {
  console.error(`matrix-workspace-tui: binary not found at ${binary}`);
  console.error('Run `npm install` (or `npm rebuild matrix-workspace-tui`) to download it.');
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' });
if (result.error) {
  console.error(`matrix-workspace-tui: failed to launch: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 1);
```

- [ ] **Step 4: Write the failing test — `npm/matrix-workspace-tui/test/launcher.test.js`**

```js
'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { getPlatform } = require('../scripts/platform');

test('launcher execs the platform binary and forwards args and exit code', () => {
  const { binaryName } = getPlatform();
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'mwt-launch-'));
  const binDir = path.join(dir, 'bin');
  fs.mkdirSync(binDir, { recursive: true });

  // Fake "binary": a shell script that echoes its args and exits 7.
  const fake = path.join(binDir, binaryName);
  fs.writeFileSync(fake, '#!/bin/sh\necho "fake-launched $1"\nexit 7\n');
  fs.chmodSync(fake, 0o755);

  fs.copyFileSync(path.join(__dirname, '..', 'index.js'), path.join(dir, 'index.js'));
  fs.cpSync(path.join(__dirname, '..', 'scripts'), path.join(dir, 'scripts'), { recursive: true });

  let output;
  let status = 0;
  try {
    output = execFileSync(process.execPath, ['index.js', 'hello'], { cwd: dir, encoding: 'utf8' });
  } catch (error) {
    output = error.stdout;
    status = error.status;
  }
  assert.match(output, /fake-launched hello/);
  assert.strictEqual(status, 7, 'the binary exit code is forwarded');
});

test('launcher prints a helpful error when the binary is missing', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'mwt-launch-'));
  fs.copyFileSync(path.join(__dirname, '..', 'index.js'), path.join(dir, 'index.js'));
  fs.cpSync(path.join(__dirname, '..', 'scripts'), path.join(dir, 'scripts'), { recursive: true });

  let output = '';
  let status = 0;
  try {
    execFileSync(process.execPath, ['index.js'], { cwd: dir, encoding: 'utf8' });
  } catch (error) {
    output = error.stderr;
    status = error.status;
  }
  assert.match(output, /binary not found/);
  assert.notStrictEqual(status, 0);
});
```

- [ ] **Step 5: Run the test to see it fail**

Run: `cd npm/matrix-workspace-tui && node --test test/launcher.test.js`
Expected: FAIL — the launcher does not exist yet (`ENOENT: no such file or directory ... index.js`). Once Step 3's `index.js` is written, the tests pass. (Write the failing test first, observe the ENOENT failure, then add `index.js`.)

- [ ] **Step 6: Commit**

```bash
git add npm/matrix-workspace-tui/package.json npm/matrix-workspace-tui/scripts/platform.js npm/matrix-workspace-tui/index.js npm/matrix-workspace-tui/test/launcher.test.js
git commit -m "feat(npm): launcher package skeleton and platform helper"
```

### Task 9.2: Download script — platform URL, checksum, install

**Files:**
- Create: `npm/matrix-workspace-tui/scripts/download.js`

- [ ] **Step 1: Write the failing test — `npm/matrix-workspace-tui/test/download.test.js`**

```js
'use strict';

const { test } = require('node:test');
const assert = require('node:assert');
const { spawnSync } = require('node:child_process');
const crypto = require('node:crypto');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');
const { getPlatform } = require('../scripts/platform');

function sha256hex(buffer) {
  return crypto.createHash('sha256').update(buffer).digest('hex');
}

/** Static server serving /<binaryName> and /<binaryName>.sha256. */
function startServer({ binaryBuffer, checksumLine }) {
  const { binaryName } = getPlatform();
  const hits = { binary: 0, checksum: 0 };
  const server = http.createServer((request, response) => {
    if (request.url === `/${binaryName}`) {
      hits.binary += 1;
      response.writeHead(200, { 'content-type': 'application/octet-stream' });
      response.end(binaryBuffer);
      return;
    }
    if (request.url === `/${binaryName}.sha256`) {
      hits.checksum += 1;
      response.writeHead(200, { 'content-type': 'text/plain' });
      response.end(checksumLine);
      return;
    }
    response.writeHead(404);
    response.end('not found');
  });
  return new Promise((resolve) => {
    server.listen(0, '127.0.0.1', () => {
      resolve({
        baseUrl: `http://127.0.0.1:${server.address().port}`,
        close: () => new Promise((done) => server.close(done)),
        hits,
      });
    });
  });
}

function runDownload(env) {
  const script = path.join(__dirname, '..', 'scripts', 'download.js');
  return spawnSync(process.execPath, [script], { encoding: 'utf8', env: { ...process.env, ...env } });
}

test('download installs the binary and verifies the sha256 checksum', async () => {
  const binaryBuffer = Buffer.from('#!/bin/sh\necho fake-binary\n');
  const checksumLine = `${sha256hex(binaryBuffer)}  matrix-workspace-tui-${getPlatform().name}`;
  const server = await startServer({ binaryBuffer, checksumLine });
  try {
    const result = runDownload({
      MATRIX_WORKSPACE_TUI_DOWNLOAD_BASE_URL: server.baseUrl,
      MATRIX_WORKSPACE_TUI_VERSION: '0.1.0',
    });
    assert.strictEqual(result.status, 0, result.stderr);
    const installed = path.join(__dirname, '..', 'bin', getPlatform().binaryName);
    assert.ok(fs.existsSync(installed), 'binary installed');
    assert.deepStrictEqual(fs.readFileSync(installed), binaryBuffer);
    assert.strictEqual(fs.statSync(installed).mode & 0o111, 0o111, 'binary is executable');
    assert.strictEqual(server.hits.binary, 1);
    assert.strictEqual(server.hits.checksum, 1);
    fs.rmSync(path.join(__dirname, '..', 'bin'), { recursive: true, force: true });
  } finally {
    await server.close();
  }
});

test('download fails hard on checksum mismatch and leaves no binary', async () => {
  const binaryBuffer = Buffer.from('#!/bin/sh\necho fake-binary\n');
  const server = await startServer({
    binaryBuffer,
    checksumLine: `${'0'.repeat(64)}  matrix-workspace-tui-${getPlatform().name}`,
  });
  try {
    const result = runDownload({
      MATRIX_WORKSPACE_TUI_DOWNLOAD_BASE_URL: server.baseUrl,
      MATRIX_WORKSPACE_TUI_VERSION: '0.1.0',
    });
    assert.notStrictEqual(result.status, 0, 'must exit non-zero');
    assert.match(result.stderr, /Checksum mismatch/);
    const binDir = path.join(__dirname, '..', 'bin');
    assert.ok(!fs.existsSync(binDir), 'no partial binary left behind');
  } finally {
    await server.close();
  }
});

test('download is idempotent when the binary already exists', async () => {
  const binaryBuffer = Buffer.from('#!/bin/sh\necho fake-binary\n');
  const checksumLine = `${sha256hex(binaryBuffer)}  matrix-workspace-tui-${getPlatform().name}`;
  const server = await startServer({ binaryBuffer, checksumLine });
  try {
    runDownload({ MATRIX_WORKSPACE_TUI_DOWNLOAD_BASE_URL: server.baseUrl, MATRIX_WORKSPACE_TUI_VERSION: '0.1.0' });
    const result = runDownload({
      MATRIX_WORKSPACE_TUI_DOWNLOAD_BASE_URL: server.baseUrl,
      MATRIX_WORKSPACE_TUI_VERSION: '0.1.0',
    });
    assert.strictEqual(result.status, 0, result.stderr);
    assert.match(result.stdout, /already present/);
    assert.strictEqual(server.hits.binary, 1, 'second run must not re-download');
    fs.rmSync(path.join(__dirname, '..', 'bin'), { recursive: true, force: true });
  } finally {
    await server.close();
  }
});
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cd npm/matrix-workspace-tui && node --test test/download.test.js`
Expected: FAIL — `download.js` does not exist (`ENOENT`).

- [ ] **Step 3: Write the implementation — full `npm/matrix-workspace-tui/scripts/download.js`**

```js
#!/usr/bin/env node
'use strict';

const { createHash } = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { getPlatform } = require('./platform');

const pkg = require('../package.json');
const VERSION = process.env.MATRIX_WORKSPACE_TUI_VERSION || pkg.version;
// Overridable so tests can point at a local static server.
const BASE_URL =
  process.env.MATRIX_WORKSPACE_TUI_DOWNLOAD_BASE_URL ||
  `https://github.com/abderrazzakchabab/matrix-workspace-tui/releases/download/v${VERSION}`;

const BIN_DIR = path.join(__dirname, '..', 'bin');
const { name, binaryName } = getPlatform();
const BINARY_PATH = path.join(BIN_DIR, binaryName);

function sha256File(filePath) {
  return new Promise((resolve, reject) => {
    const hash = createHash('sha256');
    const stream = fs.createReadStream(filePath);
    stream.on('error', reject);
    stream.on('data', (chunk) => hash.update(chunk));
    stream.on('end', () => resolve(hash.digest('hex')));
  });
}

function fetchBinary(url, destination) {
  return new Promise((resolve, reject) => {
    const client = url.startsWith('https:') ? require('node:https') : require('node:http');
    const request = client.get(url, (response) => {
      if (response.statusCode !== 200) {
        response.resume();
        reject(new Error(`Download failed: ${response.statusCode} for ${url}`));
        return;
      }
      const file = fs.createWriteStream(destination);
      response.pipe(file);
      file.on('finish', () => file.close(() => resolve()));
      file.on('error', reject);
    });
    request.on('error', reject);
  });
}

async function main() {
  try {
    if (fs.existsSync(BINARY_PATH)) {
      console.log(`matrix-workspace-tui: ${binaryName} already present, skipping download`);
      return;
    }
    fs.mkdirSync(BIN_DIR, { recursive: true });
    const binaryUrl = `${BASE_URL}/${binaryName}`;
    const checksumUrl = `${binaryUrl}.sha256`;
    const checksumPath = path.join(BIN_DIR, `${binaryName}.sha256`);
    const tmpPath = `${BINARY_PATH}.tmp`;

    console.log(`matrix-workspace-tui: downloading ${binaryName} v${VERSION}`);
    await fetchBinary(binaryUrl, tmpPath);
    await fetchBinary(checksumUrl, checksumPath);

    const expected = (await fs.promises.readFile(checksumPath, 'utf8'))
      .trim()
      .split(/\s+/)[0]
      .toLowerCase();
    const actual = await sha256File(tmpPath);
    if (expected !== actual) {
      await fs.promises.unlink(tmpPath).catch(() => {});
      throw new Error(`Checksum mismatch for ${binaryName}: expected ${expected}, got ${actual}`);
    }
    await fs.promises.rename(tmpPath, BINARY_PATH);
    await fs.promises.chmod(BINARY_PATH, 0o755);
    console.log(`matrix-workspace-tui: installed ${binaryName}`);
  } catch (error) {
    console.error(`matrix-workspace-tui: ${error.message}`);
    process.exit(1);
  }
}

main();
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cd npm/matrix-workspace-tui && node --test test/`
Expected: `# pass 5` (2 launcher + 3 download), `# fail 0`.

- [ ] **Step 5: Commit**

```bash
git add npm/matrix-workspace-tui/scripts/download.js npm/matrix-workspace-tui/test/download.test.js
git commit -m "feat(npm): checksummed platform binary download"
```

---

# Group 10: CI

### Task 10.1: `ci.yml` — test + build per platform

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Write `.github/workflows/ci.yml`**

```yaml
name: ci

on:
  push:
    branches: [main]
  pull_request:

jobs:
  test:
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            platform: linux-x64
          - os: ubuntu-24.04-arm
            target: aarch64-unknown-linux-gnu
            platform: linux-arm64
          - os: macos-latest
            target: x86_64-apple-darwin
            platform: darwin-x64
          - os: macos-14
            target: aarch64-apple-darwin
            platform: darwin-arm64
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust (pinned by rust-toolchain.toml)
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - name: Rust tests
        run: cargo test --workspace
      - name: Release build
        run: cargo build --release --target ${{ matrix.target }}
      - name: npm launcher tests
        working-directory: npm/matrix-workspace-tui
        run: npm test
```

Notes for the implementer:
- `dtolnay/rust-toolchain@stable` with no `toolchain` input uses the pinned `rust-toolchain.toml` (1.85.0).
- `ubuntu-24.04-arm` is GitHub's arm64 Linux runner; `macos-14` is the arm64 macOS runner. If either runner label is unavailable in your org, drop that row and note it — the npm package still covers the four platform names.
- The npm tests spin up a local `http` server only; no external network is needed.

- [ ] **Step 2: Validate the YAML parses**

Run: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml')); print('ci.yml OK')"`
Expected: `ci.yml OK` (PyYAML may warn about `on` as a boolean key in YAML 1.1 — that warning is harmless for GitHub Actions; the file parses.)

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: test and build on four platforms"
```

### Task 10.2: `release.yml` — four binaries → GitHub release → npm publish

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Write `.github/workflows/release.yml`**

```yaml
name: release

on:
  push:
    tags: ['v*']

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            platform: linux-x64
          - os: ubuntu-24.04-arm
            target: aarch64-unknown-linux-gnu
            platform: linux-arm64
          - os: macos-latest
            target: x86_64-apple-darwin
            platform: darwin-x64
          - os: macos-14
            target: aarch64-apple-darwin
            platform: darwin-arm64
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust (pinned by rust-toolchain.toml)
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - name: Build release binary
        run: cargo build --release --target ${{ matrix.target }}
      - name: Package binary + checksum
        run: |
          cp target/${{ matrix.target }}/release/matrix-workspace-tui matrix-workspace-tui-${{ matrix.platform }}
          sha256sum matrix-workspace-tui-${{ matrix.platform }} > matrix-workspace-tui-${{ matrix.platform }}.sha256
      - uses: actions/upload-artifact@v4
        with:
          name: binary-${{ matrix.platform }}
          path: matrix-workspace-tui-${{ matrix.platform }}*

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Download all binaries
        uses: actions/download-artifact@v4
        with:
          path: artifacts
      - name: Attach binaries to the GitHub release
        uses: softprops/action-gh-release@v2
        with:
          files: artifacts/*/*
          generate_release_notes: true
      - uses: actions/setup-node@v4
        with:
          node-version: 20
          registry-url: 'https://registry.npmjs.org'
      - name: Publish npm package
        working-directory: npm/matrix-workspace-tui
        run: npm publish
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
```

- [ ] **Step 2: Validate the YAML parses**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml')); print('release.yml OK')"`
Expected: `release.yml OK`

- [ ] **Step 3: Note the npm secret requirement (add to the plan's companion README section below if desired — for now this is a code comment in the workflow)**

The workflow publishes `npm/matrix-workspace-tui` on every `v*` tag. **The `NPM_TOKEN` secret must be configured in the repository settings** (Settings → Secrets and variables → Actions) with an npm access token that has publish rights to the `matrix-workspace-tui` package scope. The `package.json` `version` must match the release tag (e.g. tag `v0.1.0` ↔ `"version": "0.1.0"`) because `postinstall` downloads `.../releases/download/v<version>/...` at install time.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: release workflow with binaries and npm publish"
```

### Task 10.3: Final verification gate

**Files:**
- none (verification only)

- [ ] **Step 1: Run every check from a clean tree**

Run:
```bash
cargo test --workspace
cargo build --workspace --release
cd npm/matrix-workspace-tui && node --test test/
cd ../.. && git status --porcelain
```
Expected:
- `test result: ok.` for api-client (52), state (25), tui (40).
- `Finished \`release\` profile` from the release build.
- `# pass 5` / `# fail 0` from the npm tests.
- `git status --porcelain` shows nothing except untracked `target/` (ignored) — no modified tracked files.

- [ ] **Step 2: Confirm the commit log tells the whole story**

Run: `git log --oneline -10`
Expected: the most recent commits are the CI commits from this group, then the screens, app shell, state, api-client, and scaffolding commits — all on `fm/matrix-tui-plan-001`.

- [ ] **Step 3: Final commit if anything drifted**

```bash
git add -A
git commit -m "chore: final verification" || echo "tree is clean"
```

---

# Acceptance checklist

Run through this after Task 10.3. Every item maps to a task in this plan.

1. **Workspace scaffolding** — `Cargo.toml` workspace with `crates/api-client`, `crates/state`, `crates/tui`; `rust-toolchain.toml` pinned; `cargo build --workspace` passes. (Tasks 1.1–1.3)
2. **api-client HTTP core** — `ControlPlaneError { status, code }` with 401 → `SessionExpired`; `authenticated_request` sends Cookie + JSON body; httpmock-based tests. (Tasks 2.1–2.3)
3. **api-client resources** — one module per resource under `crates/api-client/src/api/` mirroring the mobile `ControlPlaneApi`: matrix session, workspaces, rooms + binding, runs (launch with idempotencyKey, cancel, deliveries), GitHub read pages with cursor, grants, approvals, mutations (202/200 replay), audit. All wire types mirror the real contract files. (Tasks 3.1–3.12)
4. **SSE event stream** — `EventStream` over `GET /api/runs/:runId/events?after=`, validated `RunEvent`s (id/runId/sequence/type/version/occurredAt/visibility/payload), malformed events ignored, resume from last sequence, terminal-event dedupe. (Tasks 4.1–4.7)
5. **SessionStore** — `~/.config/matrix-workspace-tui/session.json` via `dirs`, mode 0600, save/load/clear, corrupted-file handling, tempdir tests. (Tasks 5.1–5.4)
6. **Screen state machine** — `Screen` enum with Login → Workspaces → Rooms → RunComposer/Run → GitHubWorkspace; per-screen state structs; pure, I/O-free tests. (Tasks 6.1–6.8)
7. **App shell** — ratatui run loop (poll keys, draw, hjkl/arrows/Enter/q), status bar, screen router via the screen stack, session-expiry handling. (Tasks 7.1–7.6)
8. **Screens** — Login (token paste via bracketed paste), Workspaces (list + create), RoomBinding, RunComposer (prompt/mode/specialists/roomId), Run (live SSE timeline + terminal + cancel + authoritative Matrix delivery status), GitHubWorkspace (read panels + grant + explicit confirmation with scope/repo/args + audit). (Tasks 8.1–8.13)
9. **npm launcher** — `npm/matrix-workspace-tui` with `bin` entry, checksummed `postinstall` download for linux-x64/aarch64 + darwin-x64/aarch64, launcher exec, tests against a local static server. (Tasks 9.1–9.2)
10. **CI** — `ci.yml` (test + release build on ubuntu-latest, ubuntu-24.04-arm, macos-latest, macos-14) and `release.yml` (four binaries + sha256 attached to a GitHub release, npm publish gated on `NPM_TOKEN`). (Tasks 10.1–10.3)

**Contract-compliance spot checks (all verified against commit `063e2e1` of `abderrazzakchabab/matrix-agent-workspace`):**
- `WorkspaceSelection` = `{workspaceId, name, ownerId, status, createdAt}`; workspace create posts `policy: {readOnly: true, failurePolicy: 'partial', promptInjectionMode: 'fail_run'}`.
- `RoomSummary` = `{roomId, homeserverUrl, displayName: string|null, workspaceId: string|null}`; `RoomBinding` = `{roomId, workspaceId}`.
- `RunRequest` = `{prompt, mode: 'parallel'|'sequential', specialistIds: string[], roomId?, githubContext?}`; `RunResponse` = `{runId, status, roomId?, nextSequence}` with the 7 status literals.
- `RunEvent` fields and the 18 event-type literals (dots included) match `packages/contracts/src/events.ts`; terminal types = `run.completed|run.partial|run.failed|run.cancelled`.
- `GithubPage<T>` = `{items, nextCursor?}`; repository/issue/pull summaries match field-for-field.
- `GithubWriteScope` = `issues:write|pull_requests:write`; operations = `create_issue|update_issue|comment_issue|create_pr_comment`.
- Approval body = `{approvalType: 'github_mutation', scope, decision, confirmationText, commandHash}`; mutation body = `{idempotencyKey, approvalId, repository, runId?, operation, arguments}` with 200 = replay, 202 = new.
- Audit item = `{id, actorMatrixId?, scope?, repository?, operation?, approvalId?, commandId?, outcome, details, createdAt}`.
- SSE resume uses `?after=<sequence>`; frames are `id:`/`event:`/`data:` with `: heartbeat` comments; 401 → session expired; 404 → `RUN_NOT_FOUND`.
- Command hash = SHA-256 of the canonical `{"arguments":...,"operation":...}` JSON (sorted keys), byte-identical to the server's `computeCommandHash`; the plan's test vectors (`22a9632d…`, `8c8a0ab4…`) were computed against the real canonicalization.
