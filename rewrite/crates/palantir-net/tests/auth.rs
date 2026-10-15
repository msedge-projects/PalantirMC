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

use palantir_net::auth::{self, Poll, TokenSet};
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
    assert_eq!(
        poll,
        Poll::Token(TokenSet {
            access_token: "msa-token".into(),
            refresh_token: None,
            expires_in: 3600,
        })
    );
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

#[test]
fn a_refresh_carries_the_form_the_builder_writes_to_the_endpoint_it_names() {
    // The mock records a request's path and nothing else -- not its method and
    // not its body -- so the body is pinned where it can be: `refresh_form` is
    // a pure string and is asserted exactly, while the recorded path proves the
    // call reached the endpoint the caller handed it. A refresh that named the
    // wrong client id would be refused by the service, and this is the test
    // that would have to change if the form stopped naming it.
    let expected = concat!(
        "grant_type=refresh_token&client_id=client-1",
        "&refresh_token=refresh-1&scope=XboxLive.signin%20offline_access"
    );
    assert_eq!(auth::refresh_form("client-1", "refresh-1"), expected);

    let server = MockServer::start(vec![(
        "/token",
        Route::new(
            r#"{"access_token":"new-access","refresh_token":"refresh-2","expires_in":3600}"#,
        ),
    )]);
    let tokens =
        auth::refresh_session(&http(), &server.url("/token"), "client-1", "refresh-1").unwrap();
    assert_eq!(
        tokens,
        TokenSet {
            access_token: "new-access".into(),
            refresh_token: Some("refresh-2".into()),
            expires_in: 3600,
        }
    );
    assert_eq!(server.hits(), vec!["/token"]);
}

#[test]
fn a_refresh_answer_that_rotates_nothing_keeps_the_token_it_used() {
    // Microsoft rotates refresh tokens only when it chooses to. Dropping the one
    // that was just spent would end the session at the next access-token expiry
    // with no way back except another device code, so the used token is carried
    // forward through the whole refresh call, not only inside `keeping_refresh`.
    let server = MockServer::start(vec![(
        "/token",
        Route::new(r#"{"access_token":"new-access","expires_in":3600}"#),
    )]);
    let tokens =
        auth::refresh_session(&http(), &server.url("/token"), "client-1", "refresh-1").unwrap();
    assert_eq!(tokens.access_token, "new-access");
    assert_eq!(tokens.refresh_token.as_deref(), Some("refresh-1"));
    assert_eq!(tokens.expires_in, 3600);
}

#[test]
fn a_refused_refresh_is_an_error_and_never_an_empty_session() {
    // A refresh that failed must not read as "still signed in": the launcher
    // would hand the game an expired token and the player would see a stripped
    // session with nothing pointing at the real cause. The shared mock renders
    // every `always_status` other than 429 as a 404, so the 400-with-a-refusal
    // body branch is unreachable through it -- these are the statuses it can
    // serve, and both must be refusals that name the service.
    for status in [404u16, 429] {
        let mut route = Route::new("");
        route.always_status = Some(status);
        let server = MockServer::start(vec![("/token", route)]);
        let error = auth::refresh_session(&http(), &server.url("/token"), "client-1", "refresh-1")
            .unwrap_err()
            .to_string();
        assert!(error.contains(&format!("answered {status}")), "{error}");
        assert!(error.contains("the sign-in service"), "{error}");
        assert_eq!(
            server.hits(),
            vec!["/token"],
            "the refusal was not asked for"
        );
    }
}

#[test]
fn a_poll_answer_that_carries_a_refresh_token_keeps_it_in_the_set() {
    // The device-code flow asks for the `offline_access` scope, so the answer is
    // supposed to carry a refresh token. Keeping it here is what makes the
    // stored session renewable later; losing it makes every session last only
    // as long as its access token.
    let answer = r#"{"access_token":"msa-token","refresh_token":"refresh-1","expires_in":3600}"#;
    let server = MockServer::start(vec![("/token", Route::new(answer))]);
    let poll = auth::poll_token(&http(), &server.url("/token"), "client-id", "secret").unwrap();
    assert_eq!(
        poll,
        Poll::Token(TokenSet {
            access_token: "msa-token".into(),
            refresh_token: Some("refresh-1".into()),
            expires_in: 3600,
        })
    );
}
