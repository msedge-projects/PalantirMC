//! Authentication: offline sessions and the real Microsoft device-code login.
//!
//! Prism parity notes, verified against Prism Launcher's own sources
//! (`launcher/minecraft/auth/` and its `AuthFlow` step order):
//!
//! * Prism's `MinecraftAccount` supports `Offline` and `MSA` (Microsoft)
//!   account types. Offline sessions mirror its offline login: `user_type` is
//!   `"legacy"`, the access token is the sentinel `"0"`, and the legacy session
//!   id is `"token:0:<uuid>"`. UUID *generation* (offline v3 UUIDs derived from
//!   the player name) stays with the caller; this type transports the chosen
//!   `username`/`uuid` pair.
//! * Microsoft sessions mirror Prism's MSA login: `user_type` is `"msa"` and the
//!   session id is `"token:<access_token>:<uuid>"`, matching the
//!   `token:access:profile` shape the vanilla client expects in `auth_session`.
//! * The interactive flow is the chain Prism runs in `AuthFlow::AuthFlow`: the
//!   device-code grant ([`MicrosoftAuth::request_device_code`],
//!   [`MicrosoftAuth::poll`]), then Xbox Live user auth, then an XSTS
//!   authorization for `rp://api.minecraftservices.com/`, then
//!   [`MINECRAFT_LAUNCHER_LOGIN_URL`] for the game token, then entitlements and
//!   the profile. Every hop is the one Prism posts, down to the
//!   `x-xbl-contract-version: 1` header and the `"platform": "PC_LAUNCHER"`
//!   field — the launcher-login endpoint replaced the older `login_with_xbox`
//!   one, and it is what a third-party launcher has to use for the token to be
//!   accepted by the game.
//!
//! Every request goes through the [`HttpTransport`] trait, so the whole chain —
//! including the poll rules and the `XErr` wording — is unit-tested offline
//! against canned bodies instead of a live Microsoft tenant.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Microsoft identity-platform device-code endpoint (consumers tenant).
pub const MICROSOFT_DEVICE_FLOW_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";

/// Microsoft identity-platform token endpoint (consumers tenant).
pub const MICROSOFT_TOKEN_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

/// Default OAuth scope Prism requests (`XboxLive.signin` for the Xbox token
/// exchange plus `offline_access` for a refresh token).
pub const DEFAULT_MICROSOFT_SCOPE: &str = "XboxLive.signin offline_access";

/// Scope the device-code step asks for.
///
/// Prism's `MSADeviceCodeStep` spells the two scopes differently from the
/// [`DEFAULT_MICROSOFT_SCOPE`] constant (`XboxLive.SignIn XboxLive.offline_access`).
/// Microsoft accepts both; this is what the request sends so the traffic matches
/// Prism's byte for byte and a diff against it stays readable.
pub const DEVICE_CODE_SCOPE: &str = "XboxLive.SignIn XboxLive.offline_access";

/// Prism Launcher's public Azure application (client) id.
///
/// Taken from Prism's own build configuration (`CMakeLists.txt`:
/// `Launcher_MSA_CLIENT_ID`). It is a *public* client registered for the
/// device-code flow, which is what makes a third-party launcher able to sign in
/// at all; Prism lets users override it through `MSAClientIDOverride`, and
/// [`MicrosoftOAuth::new`] is the equivalent seam here.
pub const DEFAULT_MICROSOFT_CLIENT_ID: &str = "c36a9fb6-4f2a-41ff-90bd-ae7cc92031eb";

/// Xbox Live user authentication endpoint (`XboxUserStep`).
pub const XBOX_USER_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";

/// Xbox Live authorization (XSTS) endpoint (`XboxAuthorizationStep`).
pub const XBOX_XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";

/// XSTS relying party for Minecraft services (the value Prism passes).
pub const MOJANG_RELYING_PARTY: &str = "rp://api.minecraftservices.com/";

/// Minecraft launcher login endpoint (`LauncherLoginStep`).
pub const MINECRAFT_LAUNCHER_LOGIN_URL: &str = "https://api.minecraftservices.com/launcher/login";

/// Entitlements endpoint (`EntitlementsStep`); the request id is appended.
pub const MINECRAFT_ENTITLEMENTS_URL: &str =
    "https://api.minecraftservices.com/entitlements/license";

/// Minecraft profile endpoint (`MinecraftProfileStep`).
pub const MINECRAFT_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

/// Minecraft's skin service: a POST here puts a skin on the account.
///
/// The same host as the profile document, because it is the same service: what
/// the document *lists* is what a POST here changes. A body of
/// `{"variant":..,"url":..}` puts the texture the URL names on the account and
/// makes it the active one; a 200 answers with the new profile document.
pub const MINECRAFT_SKINS_URL: &str =
    "https://api.minecraftservices.com/minecraft/profile/skins";

/// The account's active skin, which a DELETE clears.
///
/// Clearing it is Minecraft's own `unequip`: the account goes back to the default
/// skin its name hashes to, which is what the reference's `unequip_skin` does.
pub const MINECRAFT_ACTIVE_SKIN_URL: &str =
    "https://api.minecraftservices.com/minecraft/profile/skins/active";

/// The account's active cape: a PUT with `{"capeId":..}` chooses one, a DELETE
/// hides it.
///
/// The id is the one the profile document's `capes[]` entries carry, which is why
/// the choice is expressed as an id rather than a URL: a cape is not uploaded, it
/// is *selected* from what the account already owns.
pub const MINECRAFT_ACTIVE_CAPE_URL: &str =
    "https://api.minecraftservices.com/minecraft/profile/capes/active";

/// Per-request timeout the live transport enforces.
pub const DEFAULT_AUTH_TIMEOUT_SECS: u64 = 30;

/// Shortest device-code poll interval accepted (Microsoft's own floor is 5 s).
pub const MIN_POLL_INTERVAL_SECS: u64 = 5;

/// Offline (no-auth) session: a player name plus a UUID string.
///
/// No network is involved; any string UUID supplied by the caller (typically
/// an offline v3 UUID) is carried through verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfflineSession {
    /// Player name (`auth_player_name`).
    pub username: String,
    /// Player UUID string (dashed or undashed; passed through verbatim).
    pub uuid: String,
}

impl OfflineSession {
    /// Create an offline session for `username`/`uuid`.
    pub fn new(username: impl Into<String>, uuid: impl Into<String>) -> Self {
        OfflineSession { username: username.into(), uuid: uuid.into() }
    }

    /// Convert into the [`palantir_core::launch::AuthSession`] launch seam.
    ///
    /// Prism parity: `user_type = "legacy"`, `access_token = "0"`,
    /// `session = "token:0:<uuid>"`, `user_properties = "{}"`.
    pub fn into_auth_session(self) -> palantir_core::launch::AuthSession {
        palantir_core::launch::AuthSession {
            player_name: self.username,
            uuid: self.uuid.clone(),
            access_token: "0".to_string(),
            session: format!("token:0:{}", self.uuid),
            user_type: "legacy".to_string(),
            user_properties: "{}".to_string(),
            demo: false,
        }
    }
}

impl From<OfflineSession> for palantir_core::launch::AuthSession {
    /// Convert an [`OfflineSession`] into a launch [`palantir_core::launch::AuthSession`]
    /// (see [`OfflineSession::into_auth_session`]).
    fn from(session: OfflineSession) -> Self {
        session.into_auth_session()
    }
}

/// Microsoft OAuth configuration.
///
/// Carries the client id and scope; [`MicrosoftAuth`] is the type that performs
/// the requests. Splitting them keeps the configuration copyable and comparable
/// (it can be written to a settings file or an account entry) while the
/// transport — which owns a connection pool — stays out of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicrosoftOAuth {
    /// Azure application (client) id.
    pub client_id: String,
    /// OAuth scope string (see [`DEFAULT_MICROSOFT_SCOPE`]).
    pub scope: String,
}

impl MicrosoftOAuth {
    /// Create an OAuth config with an explicit `client_id` and `scope`.
    pub fn new(client_id: impl Into<String>, scope: impl Into<String>) -> Self {
        MicrosoftOAuth { client_id: client_id.into(), scope: scope.into() }
    }

    /// Create an OAuth config with [`DEFAULT_MICROSOFT_SCOPE`].
    pub fn with_default_scope(client_id: impl Into<String>) -> Self {
        MicrosoftOAuth { client_id: client_id.into(), scope: DEFAULT_MICROSOFT_SCOPE.to_string() }
    }

    /// The client id Prism Launcher itself ships, used unless a setting says
    /// otherwise.
    pub fn public_client_id() -> MicrosoftOAuth {
        MicrosoftOAuth::with_default_scope(DEFAULT_MICROSOFT_CLIENT_ID)
    }

    /// Return the device-code endpoint URL.
    ///
    /// Prism parity: Prism posts `client_id` + `scope` to this endpoint to
    /// obtain the `user_code`/`verification_uri` pair shown to the user.
    pub fn device_flow_url(&self) -> String {
        MICROSOFT_DEVICE_FLOW_URL.to_string()
    }

    /// Return the token endpoint URL.
    ///
    /// Prism parity: Prism polls/posts the `device_code` grant (and later the
    /// `refresh_token` grant) against this endpoint.
    pub fn token_url(&self) -> String {
        MICROSOFT_TOKEN_URL.to_string()
    }

    /// Build the launch [`palantir_core::launch::AuthSession`] for an
    /// authenticated Microsoft account.
    ///
    /// Prism parity: `user_type = "msa"`,
    /// `session = "token:<access_token>:<uuid>"`,
    /// `user_properties = "{}"`. The `client_id`/`scope` on `self` do not
    /// affect the mapping; the method is namespaced here so callers keep the
    /// OAuth config and the session construction together.
    pub fn auth_session(
        &self,
        player_name: &str,
        uuid: &str,
        access_token: &str,
    ) -> palantir_core::launch::AuthSession {
        msa_auth_session(player_name, uuid, access_token)
    }
}

/// The launch session an MSA login produces (see
/// [`MicrosoftOAuth::auth_session`]).
pub fn msa_auth_session(
    player_name: &str,
    uuid: &str,
    access_token: &str,
) -> palantir_core::launch::AuthSession {
    palantir_core::launch::AuthSession {
        player_name: player_name.to_string(),
        uuid: uuid.to_string(),
        access_token: access_token.to_string(),
        session: format!("token:{access_token}:{uuid}"),
        user_type: "msa".to_string(),
        user_properties: "{}".to_string(),
        demo: false,
    }
}

// ---- transport ------------------------------------------------------------

/// One HTTP response: the status *and* the body.
///
/// Both are needed. The device-code poll endpoint reports "not yet" as HTTP 400
/// with `{"error": "authorization_pending"}`, and the XSTS endpoint reports a
/// banned or underage account as a 401 with a structured `XErr` — a transport
/// that turned a non-2xx into an error would throw away the only information
/// that says what actually went wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response body as text (lossy UTF-8; these endpoints answer JSON).
    pub body: String,
}

impl HttpResponse {
    /// Whether the status is 2xx.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// The requests the login chain and the skin service make.
///
/// Behind a trait so the whole flow can be exercised against recorded bodies:
/// the interesting behaviour is the *rules* (which poll result means "wait",
/// which `XErr` means "no Xbox profile", which status means the skin service
/// refused), and those are exactly the parts a live tenant makes untestable.
pub trait HttpTransport: Sync {
    /// `application/x-www-form-urlencoded` POST.
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> crate::Result<HttpResponse>;
    /// JSON POST with explicit headers.
    fn post_json(
        &self,
        url: &str,
        body: &str,
        headers: &[(&str, &str)],
    ) -> crate::Result<HttpResponse>;
    /// JSON PUT with explicit headers.
    ///
    /// The method the cape service asks for: choosing a cape is `PUT
    /// …/capes/active` with `{"capeId":..}`, where a POST is the skin service's
    /// own verb and would be the wrong request against this URL.
    fn put_json(
        &self,
        url: &str,
        body: &str,
        headers: &[(&str, &str)],
    ) -> crate::Result<HttpResponse>;
    /// DELETE with explicit headers and no body.
    ///
    /// Both "clear" operations -- no active skin, no active cape -- are this
    /// verb against the `/active` URL, and neither takes a body.
    fn delete(&self, url: &str, headers: &[(&str, &str)]) -> crate::Result<HttpResponse>;
    /// GET with explicit headers.
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> crate::Result<HttpResponse>;
}

/// Live transport backed by `reqwest::blocking`.
///
/// A non-2xx response is *not* an error here: it is returned with its status so
/// the caller can read the protocol error it carries (see [`HttpResponse`]).
#[derive(Debug, Clone)]
pub struct BlockingHttpTransport {
    client: reqwest::blocking::Client,
    timeout: Duration,
}

impl BlockingHttpTransport {
    /// Create a transport with the given per-request `timeout`.
    pub fn new(timeout: Duration) -> Self {
        BlockingHttpTransport { client: reqwest::blocking::Client::new(), timeout }
    }

    /// Create a transport with [`DEFAULT_AUTH_TIMEOUT_SECS`].
    pub fn with_default_timeout() -> Self {
        BlockingHttpTransport::new(Duration::from_secs(DEFAULT_AUTH_TIMEOUT_SECS))
    }

    /// The per-request timeout this transport enforces.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    fn finish(
        &self,
        url: &str,
        response: reqwest::blocking::Response,
    ) -> crate::Result<HttpResponse> {
        let status = response.status().as_u16();
        let body = response.text().map_err(|e| crate::Error::http(url, e.to_string()))?;
        Ok(HttpResponse { status, body })
    }
}

impl HttpTransport for BlockingHttpTransport {
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> crate::Result<HttpResponse> {
        let response = self
            .client
            .post(url)
            .form(form)
            .header("Accept", "application/json")
            .timeout(self.timeout)
            .send()
            .map_err(|e| crate::Error::http(url, e.to_string()))?;
        self.finish(url, response)
    }

    fn post_json(
        &self,
        url: &str,
        body: &str,
        headers: &[(&str, &str)],
    ) -> crate::Result<HttpResponse> {
        let mut request = self
            .client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .timeout(self.timeout);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request.send().map_err(|e| crate::Error::http(url, e.to_string()))?;
        self.finish(url, response)
    }

    fn put_json(
        &self,
        url: &str,
        body: &str,
        headers: &[(&str, &str)],
    ) -> crate::Result<HttpResponse> {
        let mut request = self
            .client
            .put(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_string())
            .timeout(self.timeout);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request.send().map_err(|e| crate::Error::http(url, e.to_string()))?;
        self.finish(url, response)
    }

    fn delete(&self, url: &str, headers: &[(&str, &str)]) -> crate::Result<HttpResponse> {
        let mut request = self.client.delete(url).timeout(self.timeout);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request.send().map_err(|e| crate::Error::http(url, e.to_string()))?;
        self.finish(url, response)
    }

    fn get(&self, url: &str, headers: &[(&str, &str)]) -> crate::Result<HttpResponse> {
        let mut request = self.client.get(url).timeout(self.timeout);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request.send().map_err(|e| crate::Error::http(url, e.to_string()))?;
        self.finish(url, response)
    }
}

/// Which request a canned [`MapTransport`] response belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Method {
    /// Form POST.
    Form,
    /// JSON POST.
    Json,
    /// JSON PUT.
    Put,
    /// DELETE.
    Delete,
    /// GET.
    Get,
}

/// In-memory transport for offline tests: canned status + body per
/// `(method, url)`, plus a log of what was actually requested.
///
/// Cloning shares the request log (it lives behind an `Arc`), so a test keeps a
/// handle, hands a clone to the flow, and still sees the traffic afterwards.
#[derive(Debug, Clone, Default)]
pub struct MapTransport {
    map: HashMap<(Method, String), HttpResponse>,
    seen: Arc<Mutex<Vec<(Method, String)>>>,
    sent: Arc<Mutex<Vec<(String, String)>>>,
}

impl MapTransport {
    /// An empty transport (every request fails the test).
    pub fn new() -> Self {
        MapTransport::default()
    }

    /// Record a form-POST response.
    pub fn insert_form(&mut self, url: &str, status: u16, body: &str) {
        self.map.insert(
            (Method::Form, url.to_string()),
            HttpResponse { status, body: body.to_string() },
        );
    }

    /// Record a JSON-POST response.
    pub fn insert_json(&mut self, url: &str, status: u16, body: &str) {
        self.map.insert(
            (Method::Json, url.to_string()),
            HttpResponse { status, body: body.to_string() },
        );
    }

    /// Record a JSON-PUT response.
    pub fn insert_put(&mut self, url: &str, status: u16, body: &str) {
        self.map.insert(
            (Method::Put, url.to_string()),
            HttpResponse { status, body: body.to_string() },
        );
    }

    /// Record a DELETE response.
    pub fn insert_delete(&mut self, url: &str, status: u16, body: &str) {
        self.map.insert(
            (Method::Delete, url.to_string()),
            HttpResponse { status, body: body.to_string() },
        );
    }

    /// Record a GET response.
    pub fn insert_get(&mut self, url: &str, status: u16, body: &str) {
        self.map.insert(
            (Method::Get, url.to_string()),
            HttpResponse { status, body: body.to_string() },
        );
    }

    /// Replace a recorded form-POST response.
    pub fn replace_form(&mut self, url: &str, status: u16, body: &str) {
        self.insert_form(url, status, body);
    }

    /// The URLs requested so far, in order.
    pub fn requested(&self) -> Vec<String> {
        self.seen
            .lock()
            .map(|seen| seen.iter().map(|(_, url)| url.clone()).collect())
            .unwrap_or_default()
    }

    /// The bodies sent so far, with the URL each went to, in order.
    ///
    /// Only the requests that *have* a body are logged: a GET and a DELETE send
    /// nothing, and an empty string per one of those would make a test that asks
    /// "what did the write send" count reads it did not make. The methods are not
    /// part of this: what a test asserts about a write is the URL and the JSON, and
    /// which verb carried them is asserted by the request being answered at all -- a
    /// POST canned for the cape URL would not answer the PUT that asks for it.
    pub fn bodies(&self) -> Vec<(String, String)> {
        self.sent.lock().map(|sent| sent.clone()).unwrap_or_default()
    }

    /// Find the canned response for `method` + `url`.
    ///
    /// An exact key wins. Failing that, a key the request *extends at a query
    /// boundary* does: Mojang's entitlements endpoint takes a fresh `requestId`
    /// per call, so a test that had to know the URL exactly could not can a
    /// response for it at all. Requiring the next character to be `?` is what
    /// stops a key for `…/profile` from answering `…/profile-something`, and the
    /// longest match wins so a specific key still beats a broader one.
    fn lookup(&self, method: Method, url: &str) -> crate::Result<HttpResponse> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push((method, url.to_string()));
        }
        if let Some(response) = self.map.get(&(method, url.to_string())) {
            return Ok(response.clone());
        }
        self.map
            .iter()
            .filter(|((canned_method, canned), _)| {
                canned_method == &method
                    && url.len() > canned.len()
                    && url.starts_with(canned.as_str())
                    && url.as_bytes()[canned.len()] == b'?'
            })
            .max_by_key(|((_, canned), _)| canned.len())
            .map(|(_, response)| response.clone())
            .ok_or_else(|| crate::Error::http(url, "no canned response for this request"))
    }
}

impl HttpTransport for MapTransport {
    fn post_form(&self, url: &str, _form: &[(&str, &str)]) -> crate::Result<HttpResponse> {
        self.lookup(Method::Form, url)
    }

    fn post_json(
        &self,
        url: &str,
        body: &str,
        _headers: &[(&str, &str)],
    ) -> crate::Result<HttpResponse> {
        if let Ok(mut sent) = self.sent.lock() {
            sent.push((url.to_string(), body.to_string()));
        }
        self.lookup(Method::Json, url)
    }

    fn get(&self, url: &str, _headers: &[(&str, &str)]) -> crate::Result<HttpResponse> {
        self.lookup(Method::Get, url)
    }

    fn put_json(
        &self,
        url: &str,
        body: &str,
        _headers: &[(&str, &str)],
    ) -> crate::Result<HttpResponse> {
        if let Ok(mut sent) = self.sent.lock() {
            sent.push((url.to_string(), body.to_string()));
        }
        self.lookup(Method::Put, url)
    }

    fn delete(&self, url: &str, _headers: &[(&str, &str)]) -> crate::Result<HttpResponse> {
        self.lookup(Method::Delete, url)
    }
}

// ---- flow types -----------------------------------------------------------

/// What the device-code endpoint hands back: the code the user types in and the
/// URL they type it into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCodeResponse {
    /// Opaque code the launcher polls with (never shown to the user).
    pub device_code: String,
    /// Short code the user enters in the browser.
    pub user_code: String,
    /// URL to open.
    pub verification_uri: String,
    /// Seconds until the code expires.
    pub expires_in: u64,
    /// Seconds the launcher must wait between polls.
    pub interval: u64,
}

/// The Microsoft (MSA) token pair.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MsaToken {
    /// Bearer token for Xbox Live.
    pub access_token: String,
    /// Long-lived refresh token (`offline_access` scope).
    pub refresh_token: String,
    /// Seconds the access token lives for.
    pub expires_in: i64,
}

/// The result of one poll of the token endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollOutcome {
    /// Not yet: poll again after `interval` seconds.
    Retry {
        /// Seconds to wait before the next poll.
        interval: u64,
    },
    /// The user finished signing in.
    Authorized(MsaToken),
    /// Terminal: the code expired, or the user declined.
    Failed(String),
}

/// A signed-in Minecraft session: the game token plus the profile it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinecraftSession {
    /// Game access token (`auth_access_token`).
    pub access_token: String,
    /// Seconds the game token lives for.
    pub expires_in: i64,
    /// Profile uuid (32 hex digits, no dashes).
    pub uuid: String,
    /// Profile name (the in-game name).
    pub name: String,
    /// Whether the account owns the game. `false` is a valid state: the profile
    /// exists but no entitlement was found.
    pub entitled: bool,
}

/// A login failure with a message written for the user.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    /// The request never completed (offline, DNS, timeout, TLS).
    #[error("could not reach Microsoft: {0}")]
    Transport(String),
    /// The endpoint answered, but not with something this flow understands.
    #[error("Microsoft sent an unexpected reply: {0}")]
    Protocol(String),
    /// The account itself cannot be used (the `XErr` cases, plus a missing
    /// profile). The message is the one to show the user.
    #[error("{0}")]
    Account(String),
}

impl AuthError {
    /// Whether retrying the same request could plausibly succeed.
    pub fn retryable(&self) -> bool {
        matches!(self, AuthError::Transport(_))
    }
}

/// The friendly message for an XSTS `XErr` code.
///
/// The codes and their wording follow Prism's `XboxAuthorizationStep` (which in
/// turn copied two of them from `prismarine-auth`); the point is that a user
/// sees *why* their account cannot play instead of a raw number.
pub fn xsts_message(code: i64) -> String {
    match code {
        2148916233 => "This Microsoft account has no Xbox Live profile. Buy Minecraft: Java Edition on minecraft.net first.".to_string(),
        2148916234 => "This Microsoft account has not accepted Xbox's Terms of Service. Sign in at xbox.com and accept them.".to_string(),
        2148916235 => "Xbox Live is not available in your country, so this account is blocked.".to_string(),
        2148916236 => "This Microsoft account needs proof of age. Sign in at login.live.com to provide it.".to_string(),
        2148916237 => "This Microsoft account has used up its playtime allowance and has been blocked from signing in.".to_string(),
        2148916238 => "This Microsoft account is underage and is not linked to a family group. Set the account up at account.microsoft.com/family first.".to_string(),
        2148916227 => "This Microsoft account was banned by Xbox for violating the Community Standards.".to_string(),
        2148916229 => "This Microsoft account is restricted and your guardian has not given permission to play online.".to_string(),
        other => format!("Xbox Live refused the sign-in with error {other}."),
    }
}

// ---- the flow -------------------------------------------------------------

/// A game access token, as [`MicrosoftAuth::launcher_login`] returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameToken {
    /// Bearer token for Minecraft services.
    pub access_token: String,
    /// Seconds it lives for.
    pub expires_in: i64,
}

/// One skin an account owns, as Minecraft's own profile document lists it.
///
/// The reference reads this list through a Tauri plugin
/// (`plugin:minecraft-skins|get_available_skins`) whose Rust is not in this tree,
/// and what the plugin is a wrapper around is this: the `skins` array of
/// `api.minecraftservices.com/minecraft/profile`, one entry per skin the account
/// owns, with the `state` saying which of them is in force. `url` is on
/// `textures.minecraft.net` and is the PNG a page can draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinecraftSkin {
    /// The skin's own id, which is what equipping one names.
    pub id: String,
    /// `ACTIVE` for the skin in force, `INACTIVE` for the rest.
    pub state: String,
    /// The texture's URL: a 64x64 modern PNG, or a legacy 64x32 one.
    pub url: String,
    /// `CLASSIC` or `SLIM`, the two arm widths Minecraft draws.
    pub variant: String,
}

impl MinecraftSkin {
    /// Whether this is the skin the account is wearing.
    ///
    /// Case-insensitive because the state is a service's own word rather than a
    /// type: an answer of `active` is the same answer.
    pub fn equipped(&self) -> bool {
        self.state.eq_ignore_ascii_case("ACTIVE")
    }
}

/// One cape an account owns, from the same document's `capes` array.
///
/// A cape is the same kind of thing a skin is -- a texture on
/// `textures.minecraft.net` with a state -- and differs in the one way a reader
/// notices: it has an `alias`, which is the name Minecraft shows it under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinecraftCape {
    /// The cape's own id, which is what equipping one names.
    pub id: String,
    /// `ACTIVE` for the cape in force, `INACTIVE` for the rest.
    pub state: String,
    /// The texture's URL.
    pub url: String,
    /// What Minecraft calls it: `Migrator`, `MineCon2015`, `PurpleHeart`, and so
    /// on. Empty when the document does not name one.
    pub alias: String,
}

impl MinecraftCape {
    /// Whether this is the cape the account is wearing.
    pub fn equipped(&self) -> bool {
        self.state.eq_ignore_ascii_case("ACTIVE")
    }
}

/// One change to what an account is wearing.
///
/// The four things Minecraft's skin service can be told, as one type, because a
/// page reports a change rather than a request: which verb, which URL and whether
/// there is a body are this module's business, and a caller that had to name a
/// method and a path itself would be a second place that knows the service.
///
/// Everything here names something the account *already owns* -- a texture URL and a
/// variant out of its own profile document, or a cape's id -- so no variant of this
/// type can upload anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkinChange {
    /// Put one of the account's own skins on, named by the texture it is.
    Skin {
        /// The document's own word for the arm style (`CLASSIC` or `SLIM`).
        variant: String,
        /// The texture URL the document lists for that skin.
        url: String,
    },
    /// Take the active skin off and go back to Minecraft's default.
    NoSkin,
    /// Put one of the account's own capes on, named by its id.
    Cape {
        /// The `id` the document's `capes[]` entry carries.
        id: String,
    },
    /// Hide the cape that is on.
    NoCape,
}

/// The HTTP verb one write takes.
///
/// Private, and three variants rather than a string: a caller inside this module
/// cannot misspell `PUT`, and the public surface says `/active` and `capeId`
/// instead of a method and a path.
#[derive(Debug, Clone, Copy)]
enum Verb {
    /// The skin service's own verb for putting a skin on.
    Post,
    /// Choosing a cape.
    Put,
    /// Either "clear" operation.
    Delete,
}

/// What an account's own profile document says about its appearance.
///
/// Both lists come out of the one request [`MicrosoftAuth::profile`] already
/// makes, and that is the reason this is a read of the *profile* rather than of a
/// skins endpoint: `api.minecraftservices.com/minecraft/profile` is the document
/// Minecraft itself publishes for an account, and the reference's plugin calls
/// are wrappers around it. A document with neither key is an account that owns
/// nothing yet, which is [`MinecraftSkins::default`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MinecraftSkins {
    /// The skins the account owns, in the order Minecraft lists them.
    pub skins: Vec<MinecraftSkin>,
    /// The capes it owns.
    pub capes: Vec<MinecraftCape>,
}

impl MinecraftSkins {
    /// The skin the account is wearing, if it wears one.
    ///
    /// `None` rather than the first entry: a document with no `ACTIVE` skin is a
    /// document that does not say, and the page says that rather than drawing a
    /// skin that is not in force.
    pub fn equipped(&self) -> Option<&MinecraftSkin> {
        self.skins.iter().find(|skin| skin.equipped())
    }

    /// The cape the account is wearing, if it wears one.
    pub fn equipped_cape(&self) -> Option<&MinecraftCape> {
        self.capes.iter().find(|cape| cape.equipped())
    }

    /// Whether this account owns nothing to draw.
    pub fn is_empty(&self) -> bool {
        self.skins.is_empty() && self.capes.is_empty()
    }
}

/// Drives the Microsoft login chain.
///
/// Owns a transport and the client id/scope; one instance can serve a
/// device-code login and every later token refresh, which is what a launcher
/// wants (the refresh runs again before each launch).
pub struct MicrosoftAuth {
    transport: Box<dyn HttpTransport>,
    config: MicrosoftOAuth,
}

impl std::fmt::Debug for MicrosoftAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MicrosoftAuth")
            .field("client_id", &self.config.client_id)
            .field("scope", &self.config.scope)
            .finish_non_exhaustive()
    }
}

impl MicrosoftAuth {
    /// A flow using the live transport and [`DEFAULT_AUTH_TIMEOUT_SECS`].
    pub fn new(config: MicrosoftOAuth) -> Self {
        MicrosoftAuth {
            transport: Box::new(BlockingHttpTransport::with_default_timeout()),
            config,
        }
    }

    /// A flow using the live transport with Prism's public client id.
    pub fn with_public_client_id() -> Self {
        MicrosoftAuth::new(MicrosoftOAuth::public_client_id())
    }

    /// A flow over an injected transport (tests, or a caller that wants to reuse
    /// one connection pool).
    pub fn with_transport(config: MicrosoftOAuth, transport: Box<dyn HttpTransport>) -> Self {
        MicrosoftAuth { transport, config }
    }

    /// The OAuth configuration this flow uses.
    pub fn config(&self) -> &MicrosoftOAuth {
        &self.config
    }

    /// Step 1: ask for a device code.
    ///
    /// Prism parity: `client_id` + `scope` as a form body.
    pub fn request_device_code(&self) -> Result<DeviceCodeResponse, AuthError> {
        let url = self.config.device_flow_url();
        let response = self
            .transport
            .post_form(&url, &[("client_id", &self.config.client_id), ("scope", DEVICE_CODE_SCOPE)])
            .map_err(|e| AuthError::Transport(e.to_string()))?;
        let value = parse_object(&url, &response)?;
        if let Some((error, description)) = protocol_error(&value) {
            return Err(AuthError::Account(description.unwrap_or(error)));
        }
        let device_code = string_field(&value, "device_code")
            .ok_or_else(|| AuthError::Protocol("the device-code reply had no 'device_code'".into()))?;
        let user_code = string_field(&value, "user_code")
            .ok_or_else(|| AuthError::Protocol("the device-code reply had no 'user_code'".into()))?;
        let verification_uri = string_field(&value, "verification_uri").ok_or_else(|| {
            AuthError::Protocol("the device-code reply had no 'verification_uri'".into())
        })?;
        let expires_in = number_field(&value, "expires_in").unwrap_or(900).max(0) as u64;
        let interval = number_field(&value, "interval").unwrap_or(5).max(0) as u64;
        Ok(DeviceCodeResponse {
            device_code,
            user_code,
            verification_uri,
            expires_in,
            // Polling with no wait would be a request loop; Microsoft's own
            // floor is five seconds and is also the fallback when it says zero.
            interval: interval.max(MIN_POLL_INTERVAL_SECS),
        })
    }

    /// Step 2: one poll of the token endpoint.
    ///
    /// The three outcomes RFC 8628 defines are what a launcher needs:
    /// `authorization_pending` means "ask again", `slow_down` means "ask again,
    /// more slowly" (and by five seconds for the rest of the flow), and anything
    /// else is terminal. A transport failure is reported as
    /// [`AuthError::Transport`] so the caller can choose to keep polling — Prism
    /// does exactly that, doubling the interval first.
    pub fn poll(&self, device_code: &str, interval: u64) -> Result<PollOutcome, AuthError> {
        let url = self.config.token_url();
        let response = self
            .transport
            .post_form(
                &url,
                &[
                    ("client_id", &self.config.client_id),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ("device_code", device_code),
                ],
            )
            .map_err(|e| AuthError::Transport(e.to_string()))?;
        let value = parse_object(&url, &response)?;
        if value.get("error").is_some() || !response.is_success() {
            let (error, description) = protocol_error(&value).unwrap_or_else(|| {
                (format!("http {}", response.status), None)
            });
            return Ok(match error.as_str() {
                "authorization_pending" => PollOutcome::Retry { interval },
                // RFC 8628 §3.5: the interval grows by five seconds for this and
                // every later request.
                "slow_down" => PollOutcome::Retry { interval: interval + 5 },
                _ => PollOutcome::Failed(
                    description.unwrap_or_else(|| format!("sign-in failed ({error})")),
                ),
            });
        }
        let token = parse_msa_token(&value)?;
        Ok(PollOutcome::Authorized(token))
    }

    /// Exchange a refresh token for a fresh MSA access token.
    ///
    /// Run before each launch: the game token below it lasts a day and the MSA
    /// token under that is shorter-lived still.
    pub fn refresh(&self, refresh_token: &str) -> Result<MsaToken, AuthError> {
        let url = self.config.token_url();
        let response = self
            .transport
            .post_form(
                &url,
                &[
                    ("client_id", &self.config.client_id),
                    ("grant_type", "refresh_token"),
                    ("refresh_token", refresh_token),
                    ("scope", DEVICE_CODE_SCOPE),
                ],
            )
            .map_err(|e| AuthError::Transport(e.to_string()))?;
        let value = parse_object(&url, &response)?;
        if let Some((error, description)) = protocol_error(&value) {
            // A dead refresh token is an account problem, not a protocol one:
            // the fix is to sign in again, and the message says so.
            return Err(AuthError::Account(
                description.unwrap_or_else(|| format!("the refresh token was refused ({error})")),
            ));
        }
        if !response.is_success() {
            return Err(AuthError::Protocol(format!(
                "the token endpoint answered {}",
                response.status
            )));
        }
        parse_msa_token(&value)
    }

    /// Steps 3–7: MSA token → Xbox Live → XSTS → game token → entitlements →
    /// profile.
    pub fn finish(&self, msa: &MsaToken) -> Result<MinecraftSession, AuthError> {
        let (user_token, uhs) = self.xbox_user_token(&msa.access_token)?;
        // Prism verifies the user hash survives the XSTS hop and refuses the
        // login if it does not: the hash is what ties the game token to *this*
        // account, so a changed one means something upstream is wrong.
        let xsts_token = self.xsts_token(&uhs, &user_token)?;
        let game = self.launcher_login(&uhs, &xsts_token)?;
        let entitled = self.entitled(&game.access_token).unwrap_or(false);
        let (uuid, name) = self.profile(&game.access_token)?;
        Ok(MinecraftSession {
            access_token: game.access_token,
            expires_in: game.expires_in,
            uuid,
            name,
            entitled,
        })
    }

    /// Xbox Live user authentication (`XboxUserStep`).
    ///
    /// The `RpsTicket` is `"d=<MSA token>"`; the reply carries both the token
    /// and the user hash (`uhs`) the Minecraft hop needs.
    pub fn xbox_user_token(&self, msa_access_token: &str) -> Result<(String, String), AuthError> {
        let body = serde_json::json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={msa_access_token}"),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        })
        .to_string();
        let response = self
            .transport
            .post_json(
                XBOX_USER_AUTH_URL,
                &body,
                &[("Accept", "application/json"), ("x-xbl-contract-version", "1")],
            )
            .map_err(|e| AuthError::Transport(e.to_string()))?;
        if !response.is_success() {
            return Err(AuthError::Account(format!(
                "Xbox Live refused the Microsoft token (HTTP {})",
                response.status
            )));
        }
        let value = parse_object(XBOX_USER_AUTH_URL, &response)?;
        parse_xbox_token(&value)
            .ok_or_else(|| AuthError::Protocol("the Xbox reply had no Token/uhs".into()))
    }

    /// XSTS authorization for Minecraft services (`XboxAuthorizationStep`).
    ///
    /// `uhs` is the hash the Xbox hop produced; a reply carrying a different one
    /// is refused, as Prism refuses it.
    pub fn xsts_token(&self, uhs: &str, xbox_user_token: &str) -> Result<String, AuthError> {
        let body = serde_json::json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbox_user_token],
            },
            "RelyingParty": MOJANG_RELYING_PARTY,
            "TokenType": "JWT",
        })
        .to_string();
        let response = self
            .transport
            .post_json(
                XBOX_XSTS_AUTH_URL,
                &body,
                &[("Accept", "application/json"), ("x-xbl-contract-version", "1")],
            )
            .map_err(|e| AuthError::Transport(e.to_string()))?;
        if !response.is_success() {
            // The `XErr` body is the whole point of this hop: it is what turns a
            // "401" into "this account has no Xbox profile".
            let value: serde_json::Value =
                serde_json::from_str(&response.body).unwrap_or(serde_json::Value::Null);
            if let Some(code) = value.get("XErr").and_then(|v| v.as_i64()) {
                return Err(AuthError::Account(xsts_message(code)));
            }
            return Err(AuthError::Account(format!(
                "Xbox Live authorization failed (HTTP {})",
                response.status
            )));
        }
        let value = parse_object(XBOX_XSTS_AUTH_URL, &response)?;
        let (token, reply_uhs) = parse_xbox_token(&value)
            .ok_or_else(|| AuthError::Protocol("the XSTS reply had no Token/uhs".into()))?;
        if reply_uhs != uhs {
            return Err(AuthError::Account(
                "Xbox Live changed the account hash mid-sign-in. Not using this session."
                    .to_string(),
            ));
        }
        Ok(token)
    }

    /// The game access token (`LauncherLoginStep`).
    ///
    /// The `platform` field matters: it is what makes Microsoft issue a token a
    /// *launcher* may use on the account's behalf.
    pub fn launcher_login(&self, uhs: &str, xsts_token: &str) -> Result<GameToken, AuthError> {
        let body = serde_json::json!({
            "xtoken": format!("XBL3.0 x={uhs};{xsts_token}"),
            "platform": "PC_LAUNCHER",
        })
        .to_string();
        let response = self
            .transport
            .post_json(MINECRAFT_LAUNCHER_LOGIN_URL, &body, &[("Accept", "application/json")])
            .map_err(|e| AuthError::Transport(e.to_string()))?;
        let value = parse_object(MINECRAFT_LAUNCHER_LOGIN_URL, &response)?;
        if !response.is_success() {
            let detail = string_field(&value, "errorMessage")
                .or_else(|| string_field(&value, "error"))
                .unwrap_or_else(|| format!("HTTP {}", response.status));
            return Err(AuthError::Account(format!("Minecraft refused the sign-in: {detail}")));
        }
        let access_token = string_field(&value, "access_token").ok_or_else(|| {
            AuthError::Protocol("the game-token reply had no 'access_token'".into())
        })?;
        let expires_in = number_field(&value, "expires_in").unwrap_or(86_400);
        Ok(GameToken { access_token, expires_in })
    }

    /// Whether the account owns the game (`EntitlementsStep`).
    ///
    /// Reported as `false` rather than as an error when the answer is empty: an
    /// account with a profile but no entitlement is a state the launcher can
    /// describe, not a failure of the request.
    pub fn entitled(&self, game_access_token: &str) -> Result<bool, AuthError> {
        let url = format!("{MINECRAFT_ENTITLEMENTS_URL}?requestId={}", uuid::Uuid::new_v4());
        let bearer = format!("Bearer {game_access_token}");
        let response = self
            .transport
            .get(&url, &[("Accept", "application/json"), ("Authorization", &bearer)])
            .map_err(|e| AuthError::Transport(e.to_string()))?;
        if !response.is_success() {
            return Ok(false);
        }
        let value: serde_json::Value =
            serde_json::from_str(&response.body).map_err(|e| AuthError::Protocol(e.to_string()))?;
        Ok(value
            .get("items")
            .and_then(|items| items.as_array())
            .map(|items| !items.is_empty())
            .unwrap_or(false))
    }

    /// The profile uuid and name (`MinecraftProfileStep`).
    ///
    /// A 404 is its own answer, not an error: it means the account exists but has
    /// no Java profile yet, which Prism reports as "Account has no Minecraft
    /// profile" and which the launcher shows as an account that cannot play.
    pub fn profile(&self, game_access_token: &str) -> Result<(String, String), AuthError> {
        let value = self.profile_document(game_access_token)?;
        let uuid = string_field(&value, "id")
            .ok_or_else(|| AuthError::Protocol("the profile reply had no 'id'".into()))?;
        let name = string_field(&value, "name")
            .ok_or_else(|| AuthError::Protocol("the profile reply had no 'name'".into()))?;
        Ok((uuid, name))
    }

    /// The skins and capes the account owns, from the same document.
    ///
    /// The reference's Skins page is drawn from this: the `ACTIVE` skin is the
    /// one in force, the rest are the account's own library, and the capes are
    /// the same shape. A profile with no Minecraft account behind it is the same
    /// sentence [`Self::profile`] gives rather than an empty list, because an
    /// account that cannot play has nothing to wear either.
    pub fn skins(&self, game_access_token: &str) -> Result<MinecraftSkins, AuthError> {
        let value = self.profile_document(game_access_token)?;
        Ok(MinecraftSkins { skins: skin_list(&value), capes: cape_list(&value) })
    }

    /// Change what the account is wearing, through Minecraft's own skin service.
    ///
    /// One entry point for the four writes because they share everything that is
    /// worth getting right: the same host, the same bearer, the same reading of a
    /// refusal, and the same rule that the service's error message is what the
    /// user is shown. Each is one request and no retry of its own -- the caller
    /// holds the ceiling, as it does for every request in this crate.
    ///
    /// The texture URL is the one the account's own profile document names, so an
    /// account can only put on something it already owns: there is no upload path
    /// here, which is the honest limit of this call rather than an omission.
    pub fn wear(&self, game_access_token: &str, change: SkinChange) -> Result<(), AuthError> {
        match change {
            SkinChange::Skin { variant, url } => self.wear_skin(game_access_token, &variant, &url),
            SkinChange::NoSkin => self.bare_skin(game_access_token),
            SkinChange::Cape { id } => self.wear_cape(game_access_token, &id),
            SkinChange::NoCape => self.hide_cape(game_access_token),
        }
    }

    /// Put one of the account's own skins on and make it the active one.
    ///
    /// `variant` is the document's own word for the arm style (`CLASSIC` or
    /// `SLIM`), and the service takes it lower-cased. A document that names
    /// neither -- an older account's answer has no `variant` at all -- is put on as
    /// classic, which is the arm style Minecraft itself falls back to; refusing it
    /// would refuse to wear a skin the account owns over a missing key.
    pub fn wear_skin(
        &self,
        game_access_token: &str,
        variant: &str,
        texture_url: &str,
    ) -> Result<(), AuthError> {
        if texture_url.trim().is_empty() {
            return Err(AuthError::Protocol(
                "a skin cannot be worn without naming its texture".to_string(),
            ));
        }
        let variant = if variant.eq_ignore_ascii_case("slim") { "slim" } else { "classic" };
        let body = serde_json::json!({ "variant": variant, "url": texture_url }).to_string();
        self.change(MINECRAFT_SKINS_URL, Verb::Post, &body, game_access_token)
    }

    /// Take the active skin off, which is `unequip_skin` in the reference.
    ///
    /// The account goes back to the default skin its name hashes to. Deliberately
    /// *not* "wear the first skin the document lists": the service's own answer to
    /// "nothing active" is the default, and guessing another skin would be this
    /// launcher deciding something Minecraft has a rule for.
    pub fn bare_skin(&self, game_access_token: &str) -> Result<(), AuthError> {
        self.change(MINECRAFT_ACTIVE_SKIN_URL, Verb::Delete, "", game_access_token)
    }

    /// Put on one of the account's own capes, named by the document's own id.
    pub fn wear_cape(&self, game_access_token: &str, cape_id: &str) -> Result<(), AuthError> {
        if cape_id.trim().is_empty() {
            return Err(AuthError::Protocol(
                "a cape cannot be worn without naming its id".to_string(),
            ));
        }
        let body = serde_json::json!({ "capeId": cape_id }).to_string();
        self.change(MINECRAFT_ACTIVE_CAPE_URL, Verb::Put, &body, game_access_token)
    }

    /// Hide the cape that is on, without giving up owning it.
    pub fn hide_cape(&self, game_access_token: &str) -> Result<(), AuthError> {
        self.change(MINECRAFT_ACTIVE_CAPE_URL, Verb::Delete, "", game_access_token)
    }

    /// One write to the skin service, and the one reading of its answers.
    ///
    /// Three statuses mean three different things and only one is a transport
    /// failure: 401 is a session the service no longer accepts, a 2xx is the
    /// change, and anything else carries the service's own `errorMessage` -- which
    /// is what the user is shown, because "Minecraft refused this: <its reason>"
    /// is the answer and a status number is not.
    fn change(
        &self,
        url: &str,
        verb: Verb,
        body: &str,
        game_access_token: &str,
    ) -> Result<(), AuthError> {
        let bearer = format!("Bearer {game_access_token}");
        let headers: [(&str, &str); 2] =
            [("Accept", "application/json"), ("Authorization", &bearer)];
        let response = match verb {
            Verb::Post => self.transport.post_json(url, body, &headers),
            Verb::Put => self.transport.put_json(url, body, &headers),
            Verb::Delete => self.transport.delete(url, &headers),
        }
        .map_err(|e| AuthError::Transport(e.to_string()))?;
        if response.status == 401 {
            return Err(AuthError::Account(
                "Minecraft no longer accepts this session: sign in again to change what the account wears."
                    .to_string(),
            ));
        }
        if response.is_success() {
            return Ok(());
        }
        let reason = serde_json::from_str::<serde_json::Value>(&response.body)
            .ok()
            .and_then(|value| string_field(&value, "errorMessage"))
            .unwrap_or_else(|| format!("HTTP {}", response.status));
        Err(AuthError::Account(format!("Minecraft refused the change: {reason}")))
    }

    /// The profile document itself, which both readers above share.
    ///
    /// One GET and one set of failure rules, because the uuid, the name and what
    /// the account wears all arrive in the same answer: two requests would be two
    /// chances to disagree about which account is signed in. A 404 is its own
    /// answer rather than a transport failure, for [`Self::profile`]'s reason.
    fn profile_document(&self, game_access_token: &str) -> Result<serde_json::Value, AuthError> {
        let bearer = format!("Bearer {game_access_token}");
        let response = self
            .transport
            .get(
                MINECRAFT_PROFILE_URL,
                &[("Accept", "application/json"), ("Authorization", &bearer)],
            )
            .map_err(|e| AuthError::Transport(e.to_string()))?;
        if response.status == 404 {
            return Err(AuthError::Account(
                "This Microsoft account has no Minecraft: Java Edition profile.".to_string(),
            ));
        }
        let value = parse_object(MINECRAFT_PROFILE_URL, &response)?;
        if !response.is_success() {
            return Err(AuthError::Account(format!(
                "Minecraft returned HTTP {} for the profile",
                response.status
            )));
        }
        Ok(value)
    }
}

/// The `skins` array of a profile document, as entries a page can draw.
///
/// An entry with no `url` is dropped rather than kept and drawn as a broken
/// card: the URL is the whole of what there is to draw, so an entry without one
/// is an entry that can only say so. Everything else is defaulted -- an older
/// account's answer has no `variant` -- because a missing key in a service's
/// document is not a reason to refuse the skin beside it.
fn skin_list(value: &serde_json::Value) -> Vec<MinecraftSkin> {
    let Some(entries) = value.get("skins").and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            Some(MinecraftSkin {
                url: string_field(entry, "url")?,
                id: string_field(entry, "id").unwrap_or_default(),
                state: string_field(entry, "state").unwrap_or_default(),
                variant: string_field(entry, "variant").unwrap_or_default(),
            })
        })
        .collect()
}

/// The `capes` array of a profile document, read by the same rules.
fn cape_list(value: &serde_json::Value) -> Vec<MinecraftCape> {
    let Some(entries) = value.get("capes").and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            Some(MinecraftCape {
                url: string_field(entry, "url")?,
                id: string_field(entry, "id").unwrap_or_default(),
                state: string_field(entry, "state").unwrap_or_default(),
                alias: string_field(entry, "alias").unwrap_or_default(),
            })
        })
        .collect()
}

fn parse_object(url: &str, response: &HttpResponse) -> Result<serde_json::Value, AuthError> {
    serde_json::from_str(&response.body).map_err(|e| {
        if response.is_success() {
            AuthError::Protocol(format!("{url} did not answer with JSON: {e}"))
        } else {
            AuthError::Protocol(format!(
                "{url} answered HTTP {} with a non-JSON body",
                response.status
            ))
        }
    })
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn number_field(value: &serde_json::Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|v| v.as_i64())
}

/// The `error` / `error_description` pair OAuth errors are carried in.
fn protocol_error(value: &serde_json::Value) -> Option<(String, Option<String>)> {
    let error = string_field(value, "error")?;
    Some((error, string_field(value, "error_description")))
}

fn parse_msa_token(value: &serde_json::Value) -> Result<MsaToken, AuthError> {
    let access_token = string_field(value, "access_token")
        .ok_or_else(|| AuthError::Protocol("the token reply had no 'access_token'".into()))?;
    let refresh_token = string_field(value, "refresh_token").unwrap_or_default();
    let expires_in = number_field(value, "expires_in").unwrap_or(3600);
    Ok(MsaToken { access_token, refresh_token, expires_in })
}

/// `Token` plus the first `DisplayClaims.xui[].uhs` of an Xbox reply.
fn parse_xbox_token(value: &serde_json::Value) -> Option<(String, String)> {
    let token = string_field(value, "Token")?;
    let uhs = value
        .get("DisplayClaims")?
        .get("xui")?
        .as_array()?
        .iter()
        .find_map(|claim| string_field(claim, "uhs"))?;
    Some((token, uhs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_new_stores_fields() {
        let s = OfflineSession::new("Steve", "uuid-1");
        assert_eq!(s.username, "Steve");
        assert_eq!(s.uuid, "uuid-1");
    }

    #[test]
    fn offline_into_auth_session_matches_prism_legacy_shape() {
        let s = OfflineSession::new("Steve", "uuid-1").into_auth_session();
        assert_eq!(s.player_name, "Steve");
        assert_eq!(s.uuid, "uuid-1");
        assert_eq!(s.access_token, "0");
        assert_eq!(s.session, "token:0:uuid-1");
        assert_eq!(s.user_type, "legacy");
        assert_eq!(s.user_properties, "{}");
        assert!(!s.demo);
    }

    #[test]
    fn offline_from_impl_matches_into() {
        let via_from: palantir_core::launch::AuthSession =
            OfflineSession::new("Alex", "uuid-2").into();
        let via_method = OfflineSession::new("Alex", "uuid-2").into_auth_session();
        assert_eq!(via_from, via_method);
    }

    #[test]
    fn oauth_new_stores_fields() {
        let o = MicrosoftOAuth::new("cid", "scope-a");
        assert_eq!(o.client_id, "cid");
        assert_eq!(o.scope, "scope-a");
    }

    #[test]
    fn oauth_with_default_scope_uses_constant() {
        let o = MicrosoftOAuth::with_default_scope("cid");
        assert_eq!(o.client_id, "cid");
        assert_eq!(o.scope, DEFAULT_MICROSOFT_SCOPE);
    }

    #[test]
    fn oauth_public_client_id_is_the_public_prism_app() {
        let o = MicrosoftOAuth::public_client_id();
        assert_eq!(o.client_id, DEFAULT_MICROSOFT_CLIENT_ID);
        assert_eq!(o.client_id.len(), 36, "an Azure application id is a GUID");
        assert_eq!(o.client_id.matches('-').count(), 4);
    }

    #[test]
    fn oauth_device_flow_url_matches_microsoft_endpoint() {
        let o = MicrosoftOAuth::with_default_scope("cid");
        assert_eq!(o.device_flow_url(), MICROSOFT_DEVICE_FLOW_URL);
        assert!(o.device_flow_url().contains("devicecode"));
    }

    #[test]
    fn oauth_token_url_matches_microsoft_endpoint() {
        let o = MicrosoftOAuth::with_default_scope("cid");
        assert_eq!(o.token_url(), MICROSOFT_TOKEN_URL);
        assert!(o.token_url().contains("/token"));
    }

    #[test]
    fn oauth_auth_session_matches_prism_msa_shape() {
        let o = MicrosoftOAuth::with_default_scope("cid");
        let s = o.auth_session("Steve", "uuid-9", "tok-abc");
        assert_eq!(s.player_name, "Steve");
        assert_eq!(s.uuid, "uuid-9");
        assert_eq!(s.access_token, "tok-abc");
        assert_eq!(s.session, "token:tok-abc:uuid-9");
        assert_eq!(s.user_type, "msa");
        assert!(!s.demo);
        assert_eq!(s, msa_auth_session("Steve", "uuid-9", "tok-abc"));
    }

    // ---- the flow ---------------------------------------------------------

    fn device_code_transport() -> MapTransport {
        let mut transport = MapTransport::new();
        transport.insert_form(
            MICROSOFT_DEVICE_FLOW_URL,
            200,
            r#"{"device_code":"dev-1","user_code":"ABCD-EFGH","verification_uri":"https://microsoft.com/link","expires_in":900,"interval":5,"message":"go to the link"}"#,
        );
        transport
    }

    fn flow(transport: &MapTransport) -> MicrosoftAuth {
        MicrosoftAuth::with_transport(
            MicrosoftOAuth::public_client_id(),
            Box::new(transport.clone()),
        )
    }

    /// A transport with every response the whole chain needs.
    fn full_chain_transport() -> MapTransport {
        let mut transport = device_code_transport();
        transport.insert_form(
            MICROSOFT_TOKEN_URL,
            200,
            r#"{"access_token":"msa-1","refresh_token":"refresh-1","expires_in":3600}"#,
        );
        transport.insert_json(
            XBOX_USER_AUTH_URL,
            200,
            r#"{"IssueInstant":"2026-01-01T00:00:00Z","NotAfter":"2026-01-02T00:00:00Z","Token":"user-token","DisplayClaims":{"xui":[{"uhs":"uhs-1"}]}}"#,
        );
        transport.insert_json(
            XBOX_XSTS_AUTH_URL,
            200,
            r#"{"IssueInstant":"2026-01-01T00:00:00Z","NotAfter":"2026-01-02T00:00:00Z","Token":"xsts-token","DisplayClaims":{"xui":[{"uhs":"uhs-1"}]}}"#,
        );
        transport.insert_json(
            MINECRAFT_LAUNCHER_LOGIN_URL,
            200,
            r#"{"username":"1a2b3c4d","access_token":"game-token","token_type":"Bearer","expires_in":86400}"#,
        );
        transport.insert_get(
            MINECRAFT_PROFILE_URL,
            200,
            r#"{"id":"1a2b3c4d5e6f708192a3b4c5d6e7f809","name":"Steve","skins":[]}"#,
        );
        transport
    }

    fn authorized_msa(auth: &MicrosoftAuth) -> MsaToken {
        match auth.poll("dev-1", 5).unwrap() {
            PollOutcome::Authorized(token) => token,
            other => panic!("expected a token, got {other:?}"),
        }
    }

    #[test]
    fn device_code_is_parsed_and_clamped_to_a_sane_interval() {
        let mut transport = device_code_transport();
        // Microsoft may answer `interval: 0`; polling with no wait is a request
        // loop, so the shortest accepted interval is the floor.
        transport.replace_form(
            MICROSOFT_DEVICE_FLOW_URL,
            200,
            r#"{"device_code":"dev-2","user_code":"IJKL","verification_uri":"https://microsoft.com/link","expires_in":60,"interval":0}"#,
        );
        let code = flow(&transport).request_device_code().unwrap();
        assert_eq!(code.device_code, "dev-2");
        assert_eq!(code.user_code, "IJKL");
        assert_eq!(code.verification_uri, "https://microsoft.com/link");
        assert_eq!(code.expires_in, 60);
        assert_eq!(code.interval, MIN_POLL_INTERVAL_SECS);
        assert_eq!(transport.requested(), vec![MICROSOFT_DEVICE_FLOW_URL.to_string()]);
    }

    #[test]
    fn device_code_reports_a_missing_code_instead_of_panicking() {
        let mut transport = MapTransport::new();
        transport.insert_form(MICROSOFT_DEVICE_FLOW_URL, 200, r#"{"user_code":"X"}"#);
        let err = flow(&transport).request_device_code().unwrap_err();
        assert!(matches!(err, AuthError::Protocol(_)), "got {err:?}");
        assert!(err.to_string().contains("device_code"));
    }

    #[test]
    fn device_code_surfaces_the_oauth_error_text() {
        let mut transport = MapTransport::new();
        transport.insert_form(
            MICROSOFT_DEVICE_FLOW_URL,
            400,
            r#"{"error":"invalid_client","error_description":"The client id is not registered."}"#,
        );
        let err = flow(&transport).request_device_code().unwrap_err();
        assert_eq!(err.to_string(), "The client id is not registered.");
    }

    #[test]
    fn poll_treats_pending_as_retry_and_slow_down_as_a_longer_wait() {
        let mut transport = device_code_transport();
        transport.insert_form(
            MICROSOFT_TOKEN_URL,
            400,
            r#"{"error":"authorization_pending","error_description":"still waiting"}"#,
        );
        let auth = flow(&transport);
        assert_eq!(auth.poll("dev-1", 5).unwrap(), PollOutcome::Retry { interval: 5 });

        let mut transport = device_code_transport();
        transport.insert_form(MICROSOFT_TOKEN_URL, 400, r#"{"error":"slow_down"}"#);
        let auth = flow(&transport);
        // RFC 8628 §3.5: five seconds on top of the interval in use.
        assert_eq!(auth.poll("dev-1", 10).unwrap(), PollOutcome::Retry { interval: 15 });
    }

    #[test]
    fn poll_reports_a_dead_code_as_terminal() {
        let mut transport = device_code_transport();
        transport.insert_form(
            MICROSOFT_TOKEN_URL,
            400,
            r#"{"error":"expired_token","error_description":"The code has expired."}"#,
        );
        assert_eq!(
            flow(&transport).poll("dev-1", 5).unwrap(),
            PollOutcome::Failed("The code has expired.".to_string())
        );
    }

    #[test]
    fn poll_returns_the_token_pair() {
        let mut transport = device_code_transport();
        transport.insert_form(
            MICROSOFT_TOKEN_URL,
            200,
            r#"{"access_token":"msa-1","refresh_token":"refresh-1","expires_in":3600,"token_type":"Bearer"}"#,
        );
        assert_eq!(
            flow(&transport).poll("dev-1", 5).unwrap(),
            PollOutcome::Authorized(MsaToken {
                access_token: "msa-1".into(),
                refresh_token: "refresh-1".into(),
                expires_in: 3600,
            })
        );
    }

    #[test]
    fn a_dead_refresh_token_reads_as_an_account_problem() {
        let mut transport = MapTransport::new();
        transport.insert_form(
            MICROSOFT_TOKEN_URL,
            400,
            r#"{"error":"invalid_grant","error_description":"The refresh token has expired."}"#,
        );
        let err = flow(&transport).refresh("stale").unwrap_err();
        assert!(matches!(err, AuthError::Account(_)), "got {err:?}");
        assert!(err.to_string().contains("refresh token has expired"));
        assert!(!err.retryable(), "signing in again is the only fix");
    }

    #[test]
    fn a_transport_failure_is_marked_retryable() {
        // Nothing recorded: the map transport reports a missing response the way
        // the live one reports a dead connection.
        let transport = MapTransport::new();
        let err = flow(&transport).request_device_code().unwrap_err();
        assert!(matches!(err, AuthError::Transport(_)), "got {err:?}");
        assert!(err.retryable());
    }

    #[test]
    fn the_whole_chain_produces_a_prism_shaped_session() {
        let transport = full_chain_transport();
        let auth = flow(&transport);
        let session = auth.finish(&authorized_msa(&auth)).unwrap();
        assert_eq!(session.name, "Steve");
        assert_eq!(session.uuid, "1a2b3c4d5e6f708192a3b4c5d6e7f809");
        assert_eq!(session.access_token, "game-token");
        assert_eq!(session.expires_in, 86_400);
        // No entitlements response was recorded, so ownership is reported as
        // "not established" rather than as a failure.
        assert!(!session.entitled);

        let launch = msa_auth_session(&session.name, &session.uuid, &session.access_token);
        assert_eq!(launch.user_type, "msa");
        assert_eq!(launch.session, "token:game-token:1a2b3c4d5e6f708192a3b4c5d6e7f809");

        // …and the hops are the documented ones, in order. The comparison is on
        // the endpoint rather than the whole URL, because the entitlements
        // request carries a fresh `requestId` on every call.
        let hops: Vec<String> = transport
            .requested()
            .into_iter()
            .map(|url| url.split('?').next().unwrap_or_default().to_string())
            .collect();
        assert_eq!(
            hops,
            vec![
                MICROSOFT_TOKEN_URL.to_string(),
                XBOX_USER_AUTH_URL.to_string(),
                XBOX_XSTS_AUTH_URL.to_string(),
                MINECRAFT_LAUNCHER_LOGIN_URL.to_string(),
                MINECRAFT_ENTITLEMENTS_URL.to_string(),
                MINECRAFT_PROFILE_URL.to_string(),
            ]
        );
    }

    #[test]
    fn entitlements_are_read_when_they_are_there_and_never_fail_the_login() {
        // Recorded against the endpoint: the transport matches at the query
        // boundary, so the per-call `requestId` does not have to be known here.
        let mut owned = full_chain_transport();
        owned.insert_get(
            MINECRAFT_ENTITLEMENTS_URL,
            200,
            r#"{"items":[{"name":"product_minecraft"}]}"#,
        );
        let auth = flow(&owned);
        let session = auth.finish(&authorized_msa(&auth)).unwrap();
        assert!(session.entitled, "an entitlement means ownership");

        // An empty item list is "does not own the game" — a state to describe,
        // not a failure of the request.
        let mut bare = full_chain_transport();
        bare.insert_get(MINECRAFT_ENTITLEMENTS_URL, 200, r#"{"items":[]}"#);
        let auth = flow(&bare);
        let session = auth.finish(&authorized_msa(&auth)).unwrap();
        assert!(!session.entitled);

        // …and so is an endpoint that never answers: the login still produces a
        // usable session, because ownership is not what it is asking for.
        let auth = flow(&full_chain_transport());
        let session = auth.finish(&authorized_msa(&auth)).unwrap();
        assert!(!session.entitled, "an unanswered check is not ownership");
    }

    #[test]
    fn the_xsts_hop_turns_a_401_into_the_reason() {
        let mut transport = full_chain_transport();
        transport.insert_json(XBOX_XSTS_AUTH_URL, 401, r#"{"XErr":2148916233,"Message":""}"#);
        let auth = flow(&transport);
        let err = auth.finish(&authorized_msa(&auth)).unwrap_err();
        assert!(matches!(err, AuthError::Account(_)), "got {err:?}");
        assert!(err.to_string().contains("no Xbox Live profile"), "got {err}");
        assert!(!err.retryable(), "another attempt cannot create a profile");
    }

    #[test]
    fn a_changed_account_hash_stops_the_login() {
        let mut transport = full_chain_transport();
        transport.insert_json(
            XBOX_XSTS_AUTH_URL,
            200,
            r#"{"Token":"xsts-token","DisplayClaims":{"xui":[{"uhs":"someone-else"}]}}"#,
        );
        let auth = flow(&transport);
        let err = auth.xsts_token("uhs-1", "user-token").unwrap_err();
        assert!(matches!(err, AuthError::Account(_)), "got {err:?}");
        assert!(err.to_string().contains("hash"), "got {err}");

        // …and a reply with no hash at all is a protocol problem.
        let mut transport = full_chain_transport();
        transport.insert_json(
            XBOX_XSTS_AUTH_URL,
            200,
            r#"{"Token":"xsts-token","DisplayClaims":{"xui":[]}}"#,
        );
        let auth = flow(&transport);
        let err = auth.xsts_token("uhs-1", "user-token").unwrap_err();
        assert!(matches!(err, AuthError::Protocol(_)), "got {err:?}");
    }

    #[test]
    fn a_profileless_account_is_reported_as_such() {
        let mut transport = full_chain_transport();
        transport.insert_get(MINECRAFT_PROFILE_URL, 404, r#"{"path":"/minecraft/profile"}"#);
        let err = flow(&transport).profile("game-token").unwrap_err();
        assert!(matches!(err, AuthError::Account(_)), "got {err:?}");
        assert!(err.to_string().contains("no Minecraft: Java Edition profile"));
    }

    #[test]
    fn a_profile_reply_without_a_name_is_a_protocol_error() {
        let mut transport = full_chain_transport();
        transport.insert_get(MINECRAFT_PROFILE_URL, 200, r#"{"id":"abc"}"#);
        let err = flow(&transport).profile("game-token").unwrap_err();
        assert!(matches!(err, AuthError::Protocol(_)), "got {err:?}");
    }

    /// A profile document with the appearance half filled in: three skins the
    /// account owns (the third naming no texture at all), two capes, and one of
    /// each in force.
    const DRESSED_PROFILE: &str = r#"{
        "id": "1a2b3c4d5e6f708192a3b4c5d6e7f809",
        "name": "Steve",
        "skins": [
            {"id": "skin-1", "state": "ACTIVE", "url": "http://textures.minecraft.net/texture/aaa", "variant": "CLASSIC"},
            {"id": "skin-2", "state": "INACTIVE", "url": "http://textures.minecraft.net/texture/bbb", "variant": "SLIM"},
            {"id": "skin-3", "state": "INACTIVE"}
        ],
        "capes": [
            {"id": "cape-1", "state": "INACTIVE", "url": "http://textures.minecraft.net/texture/ccc", "alias": "Migrator"},
            {"id": "cape-2", "state": "ACTIVE", "url": "http://textures.minecraft.net/texture/ddd", "alias": "MineCon2015"}
        ]
    }"#;

    #[test]
    fn the_profile_document_says_which_skin_the_account_is_wearing() {
        let mut transport = full_chain_transport();
        transport.insert_get(MINECRAFT_PROFILE_URL, 200, DRESSED_PROFILE);
        let auth = flow(&transport);
        let skins = auth.skins("game-token").expect("the appearance");
        // Three entries, two of them drawable: the third names no texture, and an
        // entry with nothing to fetch is a row that could only say so.
        assert_eq!(skins.skins.len(), 2);
        assert_eq!(skins.skins[0].id, "skin-1");
        assert_eq!(skins.skins[0].variant, "CLASSIC");
        assert_eq!(skins.skins[1].variant, "SLIM");
        assert_eq!(skins.equipped().map(|skin| skin.id.as_str()), Some("skin-1"));
        assert_eq!(skins.capes.len(), 2);
        assert_eq!(skins.equipped_cape().map(|cape| cape.alias.as_str()), Some("MineCon2015"));
        assert!(!skins.is_empty());
        // The same document still answers what a launch asks -- it is the same
        // request -- so two readers cannot disagree about which account is in.
        let (uuid, name) = auth.profile("game-token").expect("the profile");
        assert_eq!(name, "Steve");
        assert_eq!(uuid, "1a2b3c4d5e6f708192a3b4c5d6e7f809");
    }

    #[test]
    fn an_account_with_nothing_to_wear_says_so_rather_than_the_first_skin() {
        // The fixture the chain already carries: an empty `skins` array and no
        // `capes` key at all.
        let skins = flow(&full_chain_transport()).skins("game-token").expect("the appearance");
        assert!(skins.is_empty());
        assert!(skins.equipped().is_none(), "nothing active means nothing drawn");
        assert!(skins.equipped_cape().is_none());
        // A document that names neither key is the same answer, not a refusal.
        let mut transport = full_chain_transport();
        transport.insert_get(MINECRAFT_PROFILE_URL, 200, r#"{"id":"abc","name":"Steve"}"#);
        assert!(flow(&transport).skins("game-token").expect("the appearance").is_empty());
    }

    #[test]
    fn a_profileless_account_has_nothing_to_wear_either() {
        let mut transport = full_chain_transport();
        transport.insert_get(MINECRAFT_PROFILE_URL, 404, r#"{"path":"/minecraft/profile"}"#);
        let err = flow(&transport).skins("game-token").unwrap_err();
        assert!(matches!(err, AuthError::Account(_)), "got {err:?}");
        assert!(err.to_string().contains("no Minecraft: Java Edition profile"));
    }

    /// The four writes against one transport, with the service answering each
    /// canned request 2xx: what is asserted is the request itself, which is the
    /// half of a write a fixture can hold.
    fn change(transport: &MapTransport, change: SkinChange) -> Result<(), AuthError> {
        flow(transport).wear("game-token", change)
    }

    #[test]
    fn wearing_a_skin_posts_the_variant_the_service_asks_for_and_the_texture_it_is() {
        // The document says `SLIM`; the service's body is lower case. Both the
        // URL and the JSON are asserted, because a POST that sent the wrong arm
        // style would change the account's body and look like success.
        let mut transport = full_chain_transport();
        transport.insert_json(MINECRAFT_SKINS_URL, 200, DRESSED_PROFILE);
        change(
            &transport,
            SkinChange::Skin {
                variant: "SLIM".to_string(),
                url: "http://textures.minecraft.net/texture/bbb".to_string(),
            },
        )
        .expect("the skin is worn");
        assert_eq!(transport.bodies().len(), 1, "one write, and it is the skin's");
        let (url, body) = transport.bodies().remove(0);
        assert_eq!(url, MINECRAFT_SKINS_URL);
        assert!(body.contains("\"variant\":\"slim\""), "got {body}");
        assert!(body.contains("texture/bbb"), "got {body}");
        // And a variant the document does not name is put on as classic rather
        // than refused: an older account's answer has no `variant` at all.
        let mut transport = full_chain_transport();
        transport.insert_json(MINECRAFT_SKINS_URL, 200, DRESSED_PROFILE);
        change(
            &transport,
            SkinChange::Skin {
                variant: String::new(),
                url: "http://textures.minecraft.net/texture/aaa".to_string(),
            },
        )
        .expect("the skin is worn");
        let (_, body) = transport.bodies().remove(0);
        assert!(body.contains("\"variant\":\"classic\""), "got {body}");
    }

    #[test]
    fn a_skin_named_with_no_texture_is_refused_before_any_request_is_made() {
        // The one thing this pair of calls cannot do is invent a texture, so an
        // empty URL is a protocol problem *here* rather than a request the service
        // would answer with a 400 the reader would have to read.
        let transport = full_chain_transport();
        let err = change(
            &transport,
            SkinChange::Skin { variant: "CLASSIC".to_string(), url: String::new() },
        )
        .unwrap_err();
        assert!(matches!(err, AuthError::Protocol(_)), "got {err:?}");
        assert!(transport.requested().is_empty(), "nothing was sent");
        let err = change(&transport, SkinChange::Cape { id: "  ".to_string() }).unwrap_err();
        assert!(matches!(err, AuthError::Protocol(_)), "got {err:?}");
        assert!(transport.requested().is_empty());
    }

    #[test]
    fn taking_the_skin_off_and_hiding_the_cape_are_deletes_of_what_is_active() {
        let mut transport = full_chain_transport();
        transport.insert_delete(MINECRAFT_ACTIVE_SKIN_URL, 204, "");
        transport.insert_delete(MINECRAFT_ACTIVE_CAPE_URL, 204, "");
        change(&transport, SkinChange::NoSkin).expect("the skin is off");
        change(&transport, SkinChange::NoCape).expect("the cape is hidden");
        assert_eq!(
            transport.requested(),
            vec![MINECRAFT_ACTIVE_SKIN_URL.to_string(), MINECRAFT_ACTIVE_CAPE_URL.to_string()]
        );
        assert!(transport.bodies().is_empty(), "a clear sends no body");
    }

    #[test]
    fn a_cape_is_chosen_by_the_id_the_document_gave_it() {
        // The choice is an id rather than a URL because a cape is selected from
        // what the account owns, not uploaded -- so a PUT carrying the wrong shape
        // would be a request the service cannot read at all.
        let mut transport = full_chain_transport();
        transport.insert_put(MINECRAFT_ACTIVE_CAPE_URL, 200, DRESSED_PROFILE);
        change(&transport, SkinChange::Cape { id: "cape-1".to_string() }).expect("the cape is on");
        let (url, body) = transport.bodies().remove(0);
        assert_eq!(url, MINECRAFT_ACTIVE_CAPE_URL);
        assert_eq!(body, r#"{"capeId":"cape-1"}"#);
    }

    #[test]
    fn the_skin_service_s_own_reason_is_what_the_reader_is_told() {
        // 401 is a session the service no longer accepts, and it is *not* a
        // transport failure: signing in again is the fix, so retrying is not.
        let mut transport = full_chain_transport();
        transport.insert_json(MINECRAFT_SKINS_URL, 401, "");
        let err = change(
            &transport,
            SkinChange::Skin { variant: "CLASSIC".to_string(), url: "http://x/a".to_string() },
        )
        .unwrap_err();
        assert!(matches!(err, AuthError::Account(_)), "got {err:?}");
        assert!(err.to_string().contains("sign in again"), "got {err}");
        assert!(!err.retryable());
        // Any other refusal carries the service's own sentence, which is the
        // thing a reader can act on; a status number on its own is not.
        let mut transport = full_chain_transport();
        transport.insert_json(
            MINECRAFT_SKINS_URL,
            400,
            r#"{"errorMessage":"The texture is not a valid skin"}"#,
        );
        let err = change(
            &transport,
            SkinChange::Skin { variant: "CLASSIC".to_string(), url: "http://x/a".to_string() },
        )
        .unwrap_err();
        assert!(err.to_string().contains("not a valid skin"), "got {err}");
        // And a refusal with no sentence at all still says which status refused
        // it, rather than reporting success.
        let mut transport = full_chain_transport();
        transport.insert_json(MINECRAFT_SKINS_URL, 500, "not json");
        let err = change(
            &transport,
            SkinChange::Skin { variant: "CLASSIC".to_string(), url: "http://x/a".to_string() },
        )
        .unwrap_err();
        assert!(err.to_string().contains("HTTP 500"), "got {err}");
    }

    #[test]
    fn xsts_messages_cover_every_documented_code() {
        for code in [
            2148916233i64,
            2148916234,
            2148916235,
            2148916236,
            2148916237,
            2148916238,
            2148916227,
            2148916229,
        ] {
            let message = xsts_message(code);
            assert!(!message.contains(&code.to_string()), "code {code} leaked into '{message}'");
            assert!(message.ends_with('.'), "message should read as a sentence: {message}");
        }
        // Anything unknown still names the number rather than saying nothing —
        // an unrecognised refusal is still a refusal worth reporting.
        assert!(xsts_message(1).contains('1'));
    }

    #[test]
    fn the_launcher_login_reports_a_refusal_with_its_message() {
        let mut transport = full_chain_transport();
        transport.insert_json(
            MINECRAFT_LAUNCHER_LOGIN_URL,
            401,
            r#"{"error":"UNAUTHORIZED","errorMessage":"Invalid app registration"}"#,
        );
        let err = flow(&transport).launcher_login("uhs-1", "xsts-token").unwrap_err();
        assert!(err.to_string().contains("Invalid app registration"), "got {err}");
    }
}
