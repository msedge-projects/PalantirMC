//! Authentication session types (offline + Microsoft OAuth).
//!
//! Prism parity notes:
//!
//! * Prism's `MinecraftAccount` supports `Offline` and `MSA` (Microsoft)
//!   account types (`launcher/minecraft/auth/`). The interactive device-flow,
//!   token exchange, Xbox Live (`user.auth.xboxlive.com`) and Minecraft
//!   Services (`api.minecraftservices.com`) calls happen over the network in a
//!   later phase; this module only carries the *configuration* (endpoints,
//!   client id, scope) and the final conversion into
//!   [`prism_core::launch::AuthSession`] consumed by argument generation.
//! * Offline sessions mirror Prism's offline login: `user_type` is `"legacy"`
//!   (Prism uses `legacy` for offline/demo accounts and `msa` for Microsoft
//!   accounts), the access token is the sentinel `"0"`, and the legacy session
//!   id is `"token:0:<uuid>"`. UUID *generation* (offline v3 UUIDs derived from
//!   the player name) stays with the caller; this type just transports the
//!   chosen `username`/`uuid` pair.
//! * Microsoft sessions mirror Prism's MSA login: `user_type` is `"msa"` and
//!   the legacy session id is `"token:<access_token>:<uuid>"`, matching the
//!   `token:access:profile` shape the vanilla client expects in `auth_session`.

/// Microsoft identity-platform device-code endpoint (consumers tenant).
pub const MICROSOFT_DEVICE_FLOW_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";

/// Microsoft identity-platform token endpoint (consumers tenant).
pub const MICROSOFT_TOKEN_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

/// Default OAuth scope Prism requests (`XboxLive.signin` for the Xbox token
/// exchange plus `offline_access` for a refresh token).
pub const DEFAULT_MICROSOFT_SCOPE: &str = "XboxLive.signin offline_access";

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

    /// Convert into the [`prism_core::launch::AuthSession`] launch seam.
    ///
    /// Prism parity: `user_type = "legacy"`, `access_token = "0"`,
    /// `session = "token:0:<uuid>"`, `user_properties = "{}"`.
    pub fn into_auth_session(self) -> prism_core::launch::AuthSession {
        prism_core::launch::AuthSession {
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

impl From<OfflineSession> for prism_core::launch::AuthSession {
    /// Convert an [`OfflineSession`] into a launch [`prism_core::launch::AuthSession`]
    /// (see [`OfflineSession::into_auth_session`]).
    fn from(session: OfflineSession) -> Self {
        session.into_auth_session()
    }
}

/// Microsoft OAuth configuration for the device-code flow.
///
/// Only endpoint builders and the final session conversion live here; the HTTP
/// calls (device-code request, polling, Xbox/Minecraft token exchange) are a
/// later network phase built on top of [`device_flow_url`](Self::device_flow_url)
/// and [`token_url`](Self::token_url).
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

    /// Build the launch [`prism_core::launch::AuthSession`] for an
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
    ) -> prism_core::launch::AuthSession {
        prism_core::launch::AuthSession {
            player_name: player_name.to_string(),
            uuid: uuid.to_string(),
            access_token: access_token.to_string(),
            session: format!("token:{access_token}:{uuid}"),
            user_type: "msa".to_string(),
            user_properties: "{}".to_string(),
            demo: false,
        }
    }
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
        let via_from: prism_core::launch::AuthSession =
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
    }
}
