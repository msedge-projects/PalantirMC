//! Signing in: a Microsoft account, Xbox Live, and the Minecraft session it buys.
//!
//! Four steps, each one the published protocol's, in this order:
//!
//! 1. **Device code** -- the launcher asks Microsoft for a short code; the
//!    player types it on the verification page and signs in there. The
//!    launcher never sees the password, which is the whole point: a launcher
//!    that collects credentials is a launcher that can leak them.
//! 2. **Polling** -- the launcher asks for the token until the player
//!    finishes, which is what `authorization_pending` and `slow_down` mean.
//! 3. **Xbox Live** -- the Microsoft token buys an Xbox user token, then an
//!    XSTS token. Two steps because the second is the one game services
//!    accept, and it is where a child account or a blocked region fails.
//! 4. **Minecraft** -- the XSTS token buys a Minecraft access token, and the
//!    profile call is also the ownership check: an account that owns nothing
//!    gets a 404 there, which is a purchase problem and not a network one.
//!
//! Nothing here invents a token or a code. Every function takes its client id
//! and its endpoint as parameters, so a launcher with no registered client id
//! has nothing to call and must report the feature as unavailable rather than
//! display a code it made up -- a fabricated code is a lie the player acts on.
//! The parameters are also what makes the chain testable against a mock
//! service, and what lets the endpoints module be the single place the real
//! hosts are written down.

use serde::Deserialize;
use serde_json::json;

use crate::client::Http;
use crate::error::{Error, Result};

/// The sign-in chain's endpoints, as Microsoft and Mojang publish them.
///
/// The two OAuth steps are the `consumers` tenant: a launcher signs in
/// personal Microsoft accounts, and the tenant is part of the URL rather than
/// a parameter.
pub mod endpoints {
    /// Where the device code is requested.
    pub const DEVICE_CODE: &str =
        "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
    /// Where the code is exchanged for a Microsoft access token.
    pub const TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
    /// The Microsoft access token becomes an Xbox Live user token here.
    pub const XBL_USER: &str = "https://user.auth.xboxlive.com/user/authenticate";
    /// The user token becomes an XSTS token here; this is the step that
    /// refuses child accounts and blocked regions.
    pub const XSTS: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
    /// The XSTS token becomes a Minecraft access token here.
    pub const MC_LOGIN: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
    /// The profile, and the ownership check that a 404 answers.
    pub const MC_PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";
}

/// The words a no-ownership failure carries, so a caller can tell it from the
/// many other ways a request can fail. The UI offers a purchase hint for this
/// one and a retry for the rest, and the two must never merge.
const NO_OWNERSHIP: &str = "this account owns no copy of Minecraft";

/// Whether a failure means the account owns no copy of the game, rather than
/// the network or the service failing.
pub fn is_not_owned(error: &Error) -> bool {
    matches!(
        error,
        Error::Http { status: Some(404), why, .. } if why.contains(NO_OWNERSHIP)
    )
}

/// The device code a player types into Microsoft's page.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DeviceCode {
    /// What the player types. Short and case-insensitive on purpose.
    pub user_code: String,
    /// What the launcher polls with. Never shown.
    pub device_code: String,
    /// Where the player types it.
    #[serde(alias = "verification_uri_complete")]
    pub verification_uri: String,
    /// How long the code is good for, in seconds.
    pub expires_in: u64,
    /// How many seconds to wait between polls.
    #[serde(default = "default_interval")]
    pub interval: u64,
}

/// Five seconds is the protocol's own default when a service omits it.
fn default_interval() -> u64 {
    5
}

/// What a finished sign-in hands back.
///
/// The device-code request asks for the `offline_access` scope, which is
/// precisely what makes Microsoft issue a refresh token -- so a launcher that
/// kept only the access token would turn a renewable session into one that dies
/// with the token and send the player back to type a fresh code for no reason.
/// The lifetime travels with the tokens for the same reason: the session file
/// records when the token stops being usable, and it can only do that if the
/// service's own answer is carried here instead of guessed at the call site.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TokenSet {
    pub access_token: String,
    /// Absent when the service issued none; the sign-in is still usable.
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Seconds the access token is good for, as the service states it.
    #[serde(default)]
    pub expires_in: u64,
}

/// One answer to a poll. `Pending` and `SlowDown` are both "not yet", and
/// `SlowDown` additionally asks for a longer wait -- treating it as an error
/// is how a launcher gives up on a player who is simply still typing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Poll {
    /// The tokens the player's sign-in bought.
    Token(TokenSet),
    /// The player has not finished signing in.
    Pending,
    /// The player is still working; wait longer between polls.
    SlowDown,
    /// The code's lifetime ran out. A fresh code is needed.
    Expired,
    /// The player declined, or the service refused.
    Denied,
}

/// An Xbox Live token and the user hash that must travel with it: the
/// Minecraft login step puts the hash in the identity token, so losing it
/// here produces a malformed login two steps later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XboxToken {
    pub token: String,
    pub user_hash: String,
}

/// A Minecraft session: the access token the game is launched with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinecraftSession {
    pub access_token: String,
    pub expires_in: u64,
}
impl MinecraftSession {
    /// How long the service says this session lasts. The caller decides when to
    /// sign in again; the launcher never guesses a lifetime of its own.
    pub fn expires_in(&self) -> u64 {
        self.expires_in
    }
}

/// The signed-in player.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
}

/// The body of the device-code request.
pub fn device_code_form(client_id: &str, scopes: &str) -> String {
    format!(
        "client_id={}&scope={}",
        encode_form_component(client_id),
        encode_form_component(scopes)
    )
}

/// The body of a poll.
pub fn token_poll_form(client_id: &str, device_code: &str) -> String {
    format!(
        "grant_type={}&client_id={}&device_code={}",
        encode_form_component("urn:ietf:params:oauth:grant-type:device_code"),
        encode_form_component(client_id),
        encode_form_component(device_code)
    )
}

/// The body of the Xbox Live user-token step.
pub fn xbl_user_payload(msa_access_token: &str) -> String {
    json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            // The `d=` prefix marks a Microsoft account token rather than a
            // legacy one. Without it this step answers 400 and the failure
            // reads as a bad credential.
            "RpsTicket": format!("d={msa_access_token}")
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    })
    .to_string()
}

/// The body of the XSTS step. Its relying party is what the token will be
/// used for -- Minecraft's own services, not Xbox's, so this value comes from
/// the Minecraft API document rather than being interchangeable with the
/// previous step's.
pub fn xsts_payload(xbox_token: &str) -> String {
    json!({
        "Properties": {
            // RETAIL is the only sandbox a player's account has.
            "SandboxId": "RETAIL",
            "UserTokens": [xbox_token]
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    })
    .to_string()
}

/// The body of the Minecraft login step, where the user hash becomes part of
/// the identity token.
pub fn minecraft_login_payload(user_hash: &str, xsts_token: &str) -> String {
    json!({ "identityToken": format!("XBL3.0 x={user_hash};{xsts_token}") }).to_string()
}

/// What the XSTS step's refusal means, when it is one we can name. The four
/// known codes are different problems for the player (no Xbox profile, a
/// blocked region, a child account) and must not read as one message.
pub fn xsts_error(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Answer {
        #[serde(rename = "XErr")]
        xerr: Option<u64>,
    }
    let code = serde_json::from_str::<Answer>(body).ok()?.xerr?;
    Some(
        match code {
            2148916233 => {
                "this Microsoft account has no Xbox profile yet; signing in once at xbox.com \
                 (or in the Xbox app) creates one"
            }
            2148916235 => "Xbox Live is not available in this account's region",
            2148916238 => {
                "this is a child account; an adult in its Microsoft family must add it before \
                 it can sign in"
            }
            _ => "Xbox Live refused the sign-in",
        }
        .to_string(),
    )
}

/// What a poll's answer means. Kept separate from the request that produced it
/// because the interesting answers are error codes, and the offline suite's
/// mock service can serve a body or a 404 but not a 400 with a body -- so the
/// only way this mapping can be tested at all is as a function of status and
/// body.
pub fn classify_oauth_error(status: u16, body: &str) -> Result<Poll> {
    if (200..300).contains(&status) {
        // The whole set is read here, not just the access token: the refresh
        // token and the stated lifetime are what let a caller store a session
        // that outlives this process, and dropping them at the door is the one
        // place that information cannot be recovered from later.
        let tokens: TokenSet = serde_json::from_str(body).map_err(|source| Error::Invalid {
            what: "token answer",
            why: format!("{source}: {body}"),
        })?;
        return Ok(Poll::Token(tokens));
    }
    #[derive(Deserialize)]
    struct Refusal {
        error: String,
        #[serde(default)]
        error_description: String,
    }
    // A body that is not the documented refusal shape is reported as itself,
    // rather than being guessed into one of the four meanings.
    let Ok(refusal) = serde_json::from_str::<Refusal>(body) else {
        return Err(Error::Http {
            url: "the sign-in service".into(),
            status: Some(status),
            why: body.trim().to_string(),
        });
    };
    match refusal.error.as_str() {
        "authorization_pending" => Ok(Poll::Pending),
        "slow_down" => Ok(Poll::SlowDown),
        "expired_token" => Ok(Poll::Expired),
        "authorization_declined" | "authorization_denied" | "access_denied" => Ok(Poll::Denied),
        // A code we do not know is a real failure, never a silent "not yet":
        // polling an unknown error forever is how a launcher hangs instead of
        // saying what happened.
        other => Err(Error::Http {
            url: "the sign-in service".into(),
            status: Some(status),
            why: format!("{other}: {}", refusal.error_description),
        }),
    }
}

/// Ask for a device code.
pub fn request_device_code(
    http: &Http,
    endpoint: &str,
    client_id: &str,
    scopes: &str,
) -> Result<DeviceCode> {
    let response = http.post_form(endpoint, &device_code_form(client_id, scopes))?;
    let status = response.status();
    let body = response.text()?;
    if !(200..300).contains(&status) {
        return Err(Error::Http {
            url: endpoint.to_string(),
            status: Some(status),
            why: body.trim().to_string(),
        });
    }
    serde_json::from_str(&body).map_err(|source| Error::Invalid {
        what: "device-code answer",
        why: format!("{source}: {body}"),
    })
}

/// Poll once for the token. A caller that sees `SlowDown` waits longer; one
/// that sees `Pending` waits `interval`; anything else stops.
pub fn poll_token(http: &Http, endpoint: &str, client_id: &str, device_code: &str) -> Result<Poll> {
    let response = http.post_form(endpoint, &token_poll_form(client_id, device_code))?;
    let status = response.status();
    let body = response.text()?;
    classify_oauth_error(status, &body)
}

/// The body of a refresh: the refresh token from an earlier sign-in, which is
/// what buys a new access token without asking the player anything at all.
///
/// The scopes are repeated because the token endpoint answers with a token
/// scoped to what the request asked for; omitting them yields a token that
/// cannot sign in to Xbox Live, which fails two steps later instead of here.
pub fn refresh_form(client_id: &str, refresh_token: &str) -> String {
    format!(
        "grant_type={}&client_id={}&refresh_token={}&scope={}",
        encode_form_component("refresh_token"),
        encode_form_component(client_id),
        encode_form_component(refresh_token),
        encode_form_component("XboxLive.signin offline_access")
    )
}

/// Exchange a stored refresh token for a fresh set.
///
/// A refusal is reported as the service's own answer and nothing else: the
/// caller's only honest options are to fall back to a device-code sign-in or to
/// tell the player, and both need to know that the refresh failed. Treating it
/// as "still signed in" would launch the game with an expired token; treating
/// it as "signed out" would hide a real service error behind a routine screen.
///
/// A refreshed answer that carries no new refresh token keeps the one that was
/// used, which stays valid: Microsoft rotates refresh tokens only when it
/// chooses to, and dropping the old one would end the session at the next
/// access-token expiry.
pub fn refresh_session(
    http: &Http,
    endpoint: &str,
    client_id: &str,
    refresh_token: &str,
) -> Result<TokenSet> {
    let response = http.post_form(endpoint, &refresh_form(client_id, refresh_token))?;
    let status = response.status();
    let body = response.text()?;
    match classify_oauth_error(status, &body)? {
        Poll::Token(tokens) => Ok(keeping_refresh(tokens, refresh_token)),
        // The polling answers cannot come from a refresh -- "not yet" has no
        // meaning without a device code to poll -- so each one is reported as
        // the refusal it actually is rather than mapped onto a wait.
        Poll::Pending | Poll::SlowDown | Poll::Expired | Poll::Denied => Err(Error::Http {
            url: endpoint.to_string(),
            status: Some(status),
            why: format!("the session refresh was refused: {}", body.trim()),
        }),
    }
}

/// Carry the used refresh token forward when the answer offered no new one.
fn keeping_refresh(mut tokens: TokenSet, used: &str) -> TokenSet {
    if tokens.refresh_token.is_none() {
        tokens.refresh_token = Some(used.to_string());
    }
    tokens
}

/// Exchange the Microsoft access token for an Xbox Live user token.
pub fn xbox_user_token(http: &Http, msa_access_token: &str, endpoint: &str) -> Result<XboxToken> {
    let response = http.post_json(endpoint, &xbl_user_payload(msa_access_token))?;
    read_xbox_token(endpoint, response)
}

/// Exchange an Xbox Live user token for an XSTS token.
pub fn xbox_xsts_token(http: &Http, xbox_token: &XboxToken, endpoint: &str) -> Result<XboxToken> {
    let response = http.post_json(endpoint, &xsts_payload(&xbox_token.token))?;
    let status = response.status();
    let body = response.text()?;
    // The XSTS step is the one that says *why* it refused, and its reasons are
    // the player's (a child account, a region), so they are read before the
    // generic status handling.
    if !(200..300).contains(&status) {
        if let Some(why) = xsts_error(&body) {
            return Err(Error::Http {
                url: endpoint.to_string(),
                status: Some(status),
                why,
            });
        }
        return Err(Error::Http {
            url: endpoint.to_string(),
            status: Some(status),
            why: body.trim().to_string(),
        });
    }
    parse_xbox_token(endpoint, &body)
}

/// Exchange an XSTS token for a Minecraft access token.
pub fn minecraft_login(http: &Http, xsts: &XboxToken, endpoint: &str) -> Result<MinecraftSession> {
    let response = http.post_json(
        endpoint,
        &minecraft_login_payload(&xsts.user_hash, &xsts.token),
    )?;
    let status = response.status();
    let body = response.text()?;
    if !(200..300).contains(&status) {
        return Err(Error::Http {
            url: endpoint.to_string(),
            status: Some(status),
            why: body.trim().to_string(),
        });
    }
    #[derive(Deserialize)]
    struct Answer {
        access_token: String,
        #[serde(default)]
        expires_in: u64,
    }
    let answer: Answer = serde_json::from_str(&body).map_err(|source| Error::Invalid {
        what: "Minecraft session answer",
        why: format!("{source}: {body}"),
    })?;
    Ok(MinecraftSession {
        access_token: answer.access_token,
        expires_in: answer.expires_in,
    })
}

/// Read the player's profile. This is also the ownership check: an account
/// that owns no copy of the game answers 404 here, and that is a purchase
/// problem, not a broken network.
pub fn minecraft_profile(
    http: &Http,
    session: &MinecraftSession,
    endpoint: &str,
) -> Result<Profile> {
    let response = http.get_with_bearer(endpoint, &session.access_token)?;
    let status = response.status();
    let body = response.text()?;
    match status {
        200..=299 => serde_json::from_str(&body).map_err(|source| Error::Invalid {
            what: "profile answer",
            why: format!("{source}: {body}"),
        }),
        401 | 403 => Err(Error::Http {
            url: endpoint.to_string(),
            status: Some(status),
            why: "the session token was refused; sign in again".into(),
        }),
        404 => Err(Error::Http {
            url: endpoint.to_string(),
            status: Some(404),
            why: NO_OWNERSHIP.to_string(),
        }),
        other => Err(Error::Http {
            url: endpoint.to_string(),
            status: Some(other),
            why: body.trim().to_string(),
        }),
    }
}

fn read_xbox_token(endpoint: &str, response: crate::client::Response) -> Result<XboxToken> {
    let status = response.status();
    let body = response.text()?;
    if !(200..300).contains(&status) {
        return Err(Error::Http {
            url: endpoint.to_string(),
            status: Some(status),
            why: body.trim().to_string(),
        });
    }
    parse_xbox_token(endpoint, &body)
}

/// Both Xbox steps answer the same shape: a token plus `DisplayClaims.xui[0].uhs`.
fn parse_xbox_token(_endpoint: &str, body: &str) -> Result<XboxToken> {
    #[derive(Deserialize)]
    struct Answer {
        #[serde(rename = "Token")]
        token: String,
        #[serde(default, rename = "DisplayClaims")]
        display_claims: Option<DisplayClaims>,
    }
    #[derive(Deserialize)]
    struct DisplayClaims {
        #[serde(default)]
        xui: Vec<Xui>,
    }
    #[derive(Deserialize)]
    struct Xui {
        #[serde(default)]
        uhs: String,
    }
    let answer: Answer = serde_json::from_str(body).map_err(|source| Error::Invalid {
        what: "Xbox Live answer",
        why: format!("{source}: {body}"),
    })?;
    let user_hash = answer
        .display_claims
        .and_then(|claims| claims.xui.into_iter().next())
        .map(|xui| xui.uhs)
        .unwrap_or_default();
    if answer.token.is_empty() || user_hash.is_empty() {
        // A missing user hash cannot be worked around: it is half of the
        // identity token the Minecraft step needs, so this is a refusal to
        // continue rather than a token with a blank field.
        return Err(Error::Invalid {
            what: "Xbox Live answer",
            why: format!("no token or user hash in {body}"),
        });
    }
    Ok(XboxToken {
        token: answer.token,
        user_hash,
    })
}

/// Percent-encode one form component. OAuth's scope value carries spaces and
/// a colon, and a scope sent raw is a scope the service rejects.
fn encode_form_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scope_with_spaces_and_a_colon_is_encoded() {
        assert_eq!(
            device_code_form("abc-123", "XboxLive.signin offline_access"),
            "client_id=abc-123&scope=XboxLive.signin%20offline_access"
        );
        assert_eq!(
            token_poll_form("abc-123", "device+code"),
            "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code&client_id=abc-123&device_code=device%2Bcode"
        );
        // Unreserved characters are left alone, as the form encoding requires.
        assert_eq!(encode_form_component("a-Z_9.~"), "a-Z_9.~");
    }

    #[test]
    fn the_xbox_payloads_are_the_documented_shapes() {
        let user = xbl_user_payload("msa-token");
        let parsed: serde_json::Value = serde_json::from_str(&user).unwrap();
        assert_eq!(parsed["Properties"]["RpsTicket"], "d=msa-token");
        assert_eq!(parsed["Properties"]["AuthMethod"], "RPS");
        assert_eq!(parsed["RelyingParty"], "http://auth.xboxlive.com");
        assert_eq!(parsed["TokenType"], "JWT");

        let xsts = xsts_payload("xbl-token");
        let parsed: serde_json::Value = serde_json::from_str(&xsts).unwrap();
        assert_eq!(parsed["Properties"]["SandboxId"], "RETAIL");
        assert_eq!(parsed["Properties"]["UserTokens"][0], "xbl-token");
        // The relying party is Minecraft's services; the previous step's is
        // not interchangeable with it.
        assert_eq!(parsed["RelyingParty"], "rp://api.minecraftservices.com/");

        let login = minecraft_login_payload("uhs-1", "xsts-token");
        let parsed: serde_json::Value = serde_json::from_str(&login).unwrap();
        assert_eq!(parsed["identityToken"], "XBL3.0 x=uhs-1;xsts-token");
    }

    #[test]
    fn polling_answers_map_to_their_meanings() {
        assert_eq!(
            classify_oauth_error(200, r#"{"access_token":"t","expires_in":86400}"#).unwrap(),
            Poll::Token(TokenSet {
                access_token: "t".into(),
                refresh_token: None,
                expires_in: 86400,
            })
        );
        assert_eq!(
            classify_oauth_error(400, r#"{"error":"authorization_pending"}"#).unwrap(),
            Poll::Pending
        );
        assert_eq!(
            classify_oauth_error(400, r#"{"error":"slow_down"}"#).unwrap(),
            Poll::SlowDown
        );
        assert_eq!(
            classify_oauth_error(400, r#"{"error":"expired_token"}"#).unwrap(),
            Poll::Expired
        );
        for denied in ["authorization_declined", "access_denied"] {
            assert_eq!(
                classify_oauth_error(400, &format!(r#"{{"error":"{denied}"}}"#)).unwrap(),
                Poll::Denied
            );
        }
    }

    #[test]
    fn a_token_answer_keeps_the_refresh_token_it_offers() {
        let answer = classify_oauth_error(
            200,
            r#"{"access_token":"a","refresh_token":"r","expires_in":3600}"#,
        )
        .unwrap();
        assert_eq!(
            answer,
            Poll::Token(TokenSet {
                access_token: "a".into(),
                refresh_token: Some("r".into()),
                expires_in: 3600,
            })
        );
        // A service that states no lifetime or no refresh token still answered
        // a sign-in; neither absence is a reason to refuse the session.
        assert_eq!(
            classify_oauth_error(200, r#"{"access_token":"a"}"#).unwrap(),
            Poll::Token(TokenSet {
                access_token: "a".into(),
                refresh_token: None,
                expires_in: 0,
            })
        );
    }

    #[test]
    fn a_refresh_asks_for_a_scoped_token_and_keeps_a_missing_rotation() {
        let form = refresh_form("abc-123", "ref+resh");
        assert!(form.starts_with("grant_type=refresh_token&client_id=abc-123&"));
        assert!(form.contains("refresh_token=ref%2Bresh"), "{form}");
        assert!(
            form.contains("scope=XboxLive.signin%20offline_access"),
            "{form}"
        );
        let rotated = keeping_refresh(
            TokenSet {
                access_token: "new".into(),
                refresh_token: Some("next".into()),
                expires_in: 60,
            },
            "used",
        );
        assert_eq!(rotated.refresh_token.as_deref(), Some("next"));
        let unrotated = keeping_refresh(
            TokenSet {
                access_token: "new".into(),
                refresh_token: None,
                expires_in: 60,
            },
            "used",
        );
        assert_eq!(
            unrotated.refresh_token.as_deref(),
            Some("used"),
            "the token that was used is still the one a refresh needs"
        );
    }

    #[test]
    fn an_unknown_error_is_reported_and_not_polled_as_pending() {
        let error = classify_oauth_error(
            400,
            r#"{"error":"invalid_client","error_description":"bad id"}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("invalid_client"), "{error}");
        // A throttle is not "the player is still typing" either.
        assert!(classify_oauth_error(429, "").is_err());
    }

    #[test]
    fn the_xsts_refusals_name_their_causes() {
        assert!(
            xsts_error(r#"{"XErr":2148916238}"#)
                .unwrap()
                .contains("child account")
        );
        assert!(
            xsts_error(r#"{"XErr":2148916235}"#)
                .unwrap()
                .contains("region")
        );
        assert!(
            xsts_error(r#"{"XErr":2148916233}"#)
                .unwrap()
                .contains("no Xbox profile")
        );
        assert_eq!(xsts_error(r#"{"Token":"ok"}"#), None);
        assert_eq!(xsts_error("not json"), None);
    }

    #[test]
    fn an_xbox_answer_without_a_user_hash_is_refused() {
        assert!(parse_xbox_token("/x", r#"{"Token":"t"}"#).is_err());
        assert!(parse_xbox_token("/x", r#"{"Token":"t","DisplayClaims":{"xui":[]}}"#).is_err());
        let ok = parse_xbox_token(
            "/x",
            r#"{"Token":"t","DisplayClaims":{"xui":[{"uhs":"u"}]}}"#,
        )
        .unwrap();
        assert_eq!(ok.user_hash, "u");
    }
}
