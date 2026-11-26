use super::*;
use crate::test_support::{Fake, FakeServer};

const START: &str = "https://github.com/daedyx/tasks/releases/latest/download/entry.json";
const ASSET: &str = "https://release-assets.githubusercontent.com/github-production-release-asset/1?sig=a%2Fb&x=1";

fn get(server: &mut FakeServer, limit: u64) -> Result<Vec<u8>, FetchError> {
    get_bytes(server, START, limit)
}

#[test]
fn a_direct_answer_and_a_redirect_to_an_allowed_host_work() {
    let mut s = FakeServer::default().with(START, Fake::ok(b"abc"));
    assert_eq!(get(&mut s, 3).unwrap(), b"abc");
    let mut s = FakeServer::default().with(START, Fake::Redirect(ASSET.into())).with(ASSET, Fake::ok(b"xyz"));
    assert_eq!(get(&mut s, 10).unwrap(), b"xyz");
    assert_eq!(s.requests, [START, ASSET]);
}

#[test]
fn three_redirects_are_allowed_four_are_not() {
    let hop = |i: usize| format!("https://objects.githubusercontent.com/{i}");
    let mut s = FakeServer::default().with(START, Fake::Redirect(hop(1)));
    for i in 1..=3 {
        s = s.with(&hop(i), Fake::Redirect(hop(i.saturating_add(1))));
    }
    s = s.with(&hop(3), Fake::ok(b"ok"));
    assert_eq!(get(&mut s, 2).unwrap(), b"ok");
    let mut s = s.with(&hop(3), Fake::Redirect(hop(4))).with(&hop(4), Fake::ok(b"ok"));
    assert_eq!(get(&mut s, 2), Err(FetchError::TooManyRedirects));
    assert_eq!(s.requests.len(), 8, "je Abruf höchstens 4 Anfragen, nie eine fünfte");
}

#[test]
fn redirects_to_foreign_or_plain_hosts_are_refused_before_asking_them() {
    for to in [
        "https://evil.example/x",
        "https://github.com.evil.example/x",
        "https://evilgithub.com/x",
        "http://objects.githubusercontent.com/x",
        concat!("https://user", "@", "objects.githubusercontent.com/x"),
        "https://objects.githubusercontent.com:8443/x",
        "https://OBJECTS.githubusercontent.com/x",
        "/relative/path",
        "https://objects.githubusercontent.com",
        "https://objects.githubusercontent.com/a b",
        "https://objects.githubusercontent.com/a\\b",
        "https://objects.githubusercontent.com/a#b",
    ] {
        let mut s = FakeServer::default().with(START, Fake::Redirect(to.into())).with(to, Fake::ok(b"x"));
        let err = get(&mut s, 10).unwrap_err();
        assert!(matches!(err, FetchError::ForeignHost | FetchError::BadAddress), "{to}: {err:?}");
        assert_eq!(s.requests, [START], "{to}: fremder Rechner nie angefragt");
    }
    let mut s = FakeServer::default();
    assert_eq!(get_bytes(&mut s, "https://evil.example/entry.json", 1), Err(FetchError::ForeignHost));
    assert!(s.requests.is_empty());
}

#[test]
fn size_is_limited_while_loading() {
    let mut s = FakeServer::default().with(START, Fake::Body(vec![7; 11], None));
    assert_eq!(get(&mut s, 10), Err(FetchError::TooLarge));
    let mut s = FakeServer::default().with(START, Fake::Body(vec![7; 10], None));
    assert_eq!(get(&mut s, 10).unwrap().len(), 10);
    // Angekündigte Länge über der Grenze: gar nicht erst lesen.
    let mut s = FakeServer::default().with(START, Fake::Body(vec![], Some(11)));
    assert_eq!(get(&mut s, 10), Err(FetchError::TooLarge));
}

#[test]
fn the_announced_length_must_match() {
    let mut s = FakeServer::default().with(START, Fake::Body(vec![1; 5], Some(6)));
    assert_eq!(get(&mut s, 10), Err(FetchError::Interrupted));
    let mut s = FakeServer::default().with(START, Fake::Body(vec![1; 7], Some(6)));
    assert_eq!(get(&mut s, 10), Err(FetchError::Interrupted));
}

#[test]
fn network_failures_map_to_clear_reasons() {
    let cases = [
        (Fake::Fail(TransportError::Offline), FetchError::Offline),
        (Fake::Fail(TransportError::Timeout), FetchError::Timeout),
        (Fake::Fail(TransportError::Failed), FetchError::Interrupted),
        (Fake::Status(404), FetchError::Status(404)),
        (Fake::Status(500), FetchError::Status(500)),
        (Fake::BodyThenError(vec![1; 3], std::io::ErrorKind::TimedOut), FetchError::Timeout),
        (Fake::BodyThenError(vec![1; 3], std::io::ErrorKind::ConnectionReset), FetchError::Interrupted),
    ];
    for (reply, want) in cases {
        let mut s = FakeServer::default().with(START, reply);
        assert_eq!(get(&mut s, 100), Err(want.clone()), "{want:?}");
    }
}

#[test]
fn progress_reports_loaded_bytes() {
    let mut s = FakeServer::default().with(START, Fake::ok(b"hello"));
    let mut seen = Vec::new();
    let mut out = Vec::new();
    get_to(&mut s, START, 5, &mut out, &mut |n| seen.push(n)).unwrap();
    assert_eq!(seen.last(), Some(&5));
}

#[test]
fn the_url_type_keeps_host_and_text() {
    let u = HttpsUrl::parse(ASSET).unwrap();
    assert_eq!(u.host(), "release-assets.githubusercontent.com");
    assert_eq!(u.as_str(), ASSET);
    assert!(HttpsUrl::parse(&format!("https://github.com/{}", "a".repeat(9000))).is_err());
    assert!(HttpsUrl::parse("https:///x").is_err());
    assert!(HttpsUrl::parse("https://-a.com/x").is_err());
}
