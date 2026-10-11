//! The sign-in chain against a fake service.
//!
//! What these tests can and cannot prove is worth stating, because the mock
//! service records request *paths* and not headers or bodies: they prove the
//! order of the steps, which endpoint each step reached, and that each answer
//! is parsed into the right shape. The request bodies themselves are proven by
//! the pure builders in `auth.rs`, which assert their exact contents.
//!
//! One limit of the shared mock is load-bearing here: it renders any
//! `always_status` other than 429 as a 404, so the 401/403 branches of the
//! profile call cannot be exercised through it. The 404 -- the one that means
//! "this account owns nothing" -- is exercised, and is the one that matters.

use palantir_net::auth::{self, Poll};
use palantir_net::client::Http;
use test_support::{MockServer, Route};

fn http() -> Http {
    Http::new().unwrap()
}

#[test]
fn a_device_code_arrives_parsed() {
    let server = MockServer::start(vec![(
        "/devicecode",
        Route::new(
            r#"{"user_code":"ABCD-EFGH","device_code":"secret","verification_uri":
               "https://microsoft.com/link","expires_in":900,"interval":5}"#,
        ),
    )]);
    let code = auth::request_device_code(
        &http(),
        &server.url("/devicecode"),
        "client-id",
        "XboxLive.signin offline_access",
    )
    .unwrap();
    assert_eq!(code.user_code, "ABCD-EFGH");
    assert_eq!(code.device_code, "secret");
    assert_eq!(code.verification_uri, "https://microsoft.com/link");
    assert_eq!(code.expires_in, 900);
    assert_eq!(code.interval, 5);
    assert_eq!(server.hits(), vec!["/devicecode"]);
}

#[test]
fn polling_returns_the_token_when_the_service_answers() {
    let server = MockServer::start(vec![(
        "/token",
        Route::new(r#"{"access_token":"msa-token","expires_in":3600}"#),
    )]);
    let poll = auth::poll_token(&http(), &server.url("/token"), "client-id", "secret").unwrap();
    assert_eq!(poll, Poll::Token("msa-token".into()));
    assert_eq!(server.hits(), vec!["/token"]);
}

#[test]
fn a_throttled_poll_is_a_failure_and_not_a_pending() {
    let mut route = Route::new("");
    route.always_status = Some(429);
    let server = MockServer::start(vec![("/token", route)]);
    let error = auth::poll_token(&http(), &server.url("/token"), "client-id", "secret")
        .unwrap_err()
        .to_string();
    assert!(error.contains("429"), "{error}");
    // Not a "still typing" answer, and not an ownership problem either.
    assert!(!error.contains("owns no copy"), "{error}");
}

#[test]
fn the_xbox_chain_walks_the_user_token_then_the_xsts_token() {
    let claims = r#""DisplayClaims":{"xui":[{"uhs":"uhs-1"}]}"#;
    let server = MockServer::start(vec![
        (
            "/xbl",
            Route::new(format!("{{\"Token\":\"xbl-token\",{claims}}}")),
        ),
        (
            "/xsts",
            Route::new(format!("{{\"Token\":\"xsts-token\",{claims}}}")),
        ),
    ]);
    let user = auth::xbox_user_token(&http(), "msa-token", &server.url("/xbl")).unwrap();
    assert_eq!(user.token, "xbl-token");
    assert_eq!(user.user_hash, "uhs-1");

    let xsts = auth::xbox_xsts_token(&http(), &user, &server.url("/xsts")).unwrap();
    assert_eq!(xsts.token, "xsts-token");
    // The hash travels with the token: it is half of the identity token the
    // Minecraft step builds, so losing it here breaks the step after next.
    assert_eq!(xsts.user_hash, "uhs-1");
    assert_eq!(
        server.hits(),
        vec!["/xbl", "/xsts"],
        "steps ran out of order"
    );
}

#[test]
fn login_yields_the_session_the_game_is_launched_with() {
    let server = MockServer::start(vec![(
        "/login",
        Route::new(r#"{"access_token":"mc-token","expires_in":86400}"#),
    )]);
    let xsts = auth::XboxToken {
        token: "xsts-token".into(),
        user_hash: "uhs-1".into(),
    };
    let session = auth::minecraft_login(&http(), &xsts, &server.url("/login")).unwrap();
    assert_eq!(session.access_token, "mc-token");
    assert_eq!(session.expires_in, 86400);
    assert_eq!(server.hits(), vec!["/login"]);
}

#[test]
fn the_profile_call_reads_the_player() {
    let server = MockServer::start(vec![(
        "/profile",
        Route::new(r#"{"id":"069a79f4","name":"Notch"}"#),
    )]);
    let session = auth::MinecraftSession {
        access_token: "mc-token".into(),
        expires_in: 86400,
    };
    let profile = auth::minecraft_profile(&http(), &session, &server.url("/profile")).unwrap();
    assert_eq!(profile.name, "Notch");
    assert_eq!(profile.id, "069a79f4");
    // The token rode in an Authorization header; the mock records only the
    // path, so what is proven here is the endpoint and the parse.
    assert_eq!(server.hits(), vec!["/profile"]);
}

#[test]
fn a_profile_404_says_the_account_owns_nothing() {
    let mut route = Route::new("");
    route.always_status = Some(404);
    let server = MockServer::start(vec![("/profile", route)]);
    let session = auth::MinecraftSession {
        access_token: "mc-token".into(),
        expires_in: 86400,
    };
    let error = auth::minecraft_profile(&http(), &session, &server.url("/profile")).unwrap_err();
    assert!(auth::is_not_owned(&error), "{error}");
    let message = error.to_string();
    assert!(message.contains("owns no copy"), "{message}");
    assert!(message.contains("404"), "{message}");
}

#[test]
fn only_the_profile_call_reads_a_404_as_ownership() {
    // The mock answers an unrouted path with 404, which is the same status
    // the profile call gives an unowned account. If every 404 were read as
    // ownership, a moved endpoint would tell the player to buy the game.
    let server = MockServer::start(vec![]);
    let error = auth::request_device_code(
        &http(),
        &server.url("/moved"),
        "client-id",
        "XboxLive.signin",
    )
    .unwrap_err();
    assert!(!auth::is_not_owned(&error), "{error}");
    assert!(error.to_string().contains("404"), "{error}");
}
