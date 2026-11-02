use super::*;
use crate::pull_request::{by_number, parse::parse_by_number};

fn numbered(pr: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"data":{"repository":{"mergeCommitAllowed":true,"pullRequest":pr}}})
}

fn same_repository() -> serde_json::Value {
    let mut pr = response()[0].clone();
    pr["isCrossRepository"] = false.into();
    pr["headRepositoryOwner"] = serde_json::json!({"login":"example"});
    pr["headRepository"] = serde_json::json!({"name":"project"});
    pr
}

#[test]
fn any_pull_request_is_found_by_number_and_checked_against_its_own_head() {
    let pr = parse_by_number(
        numbered(same_repository()),
        &crate::Server::GitHub,
        "example",
        "project",
        8,
    )
    .unwrap()
    .unwrap();
    assert_eq!(pr.number, 8);
    assert_eq!(pr.url, "https://github.com/example/project/pull/8");
    assert_eq!(pr.merge_methods, [crate::pull_request::MergeMethod::Merge]);
    // The number asked for is the one answered.
    assert!(matches!(
        parse_by_number(
            numbered(same_repository()),
            &crate::Server::GitHub,
            "example",
            "project",
            9
        ),
        Err(Error::PullRequest(crate::pull_request::Error::Identity))
    ));
    assert!(
        parse_by_number(
            numbered(serde_json::Value::Null),
            &crate::Server::GitHub,
            "example",
            "project",
            8
        )
        .unwrap()
        .is_none()
    );
    // Another repository's pull request does not pass for this one's.
    assert!(matches!(
        parse_by_number(
            numbered(same_repository()),
            &crate::Server::GitHub,
            "example",
            "other",
            8
        ),
        Err(Error::PullRequest(crate::pull_request::Error::Identity))
    ));
}

#[test]
fn an_enterprise_pull_request_must_link_to_its_own_server() {
    let server = crate::Server::enterprise("git.example.com:8443").unwrap();
    let mut pr = same_repository();
    pr["url"] = "https://git.example.com:8443/example/project/pull/8".into();
    let found = parse_by_number(numbered(pr.clone()), &server, "example", "project", 8)
        .unwrap()
        .unwrap();
    assert_eq!(
        found.url,
        "https://git.example.com:8443/example/project/pull/8"
    );
    // The same path on GitHub.com, or a GitHub.com link from the server, is
    // another pull request.
    assert!(matches!(
        parse_by_number(
            numbered(pr),
            &crate::Server::GitHub,
            "example",
            "project",
            8
        ),
        Err(Error::PullRequest(crate::pull_request::Error::Identity))
    ));
    assert!(matches!(
        parse_by_number(
            numbered(same_repository()),
            &server,
            "example",
            "project",
            8
        ),
        Err(Error::PullRequest(crate::pull_request::Error::Identity))
    ));
}

/// Answers each GraphQL call with `status` and `body` on loopback, and
/// routes `token`'s calls to it.
fn fake_server(token: &str, status: u16, body: String) {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    crate::fake_graphql(
        token,
        format!("http://{}/api/graphql", listener.local_addr().unwrap()),
    );
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut length = 0;
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 && !line.trim_end().is_empty() {
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap_or(0);
                }
                line.clear();
            }
            let mut request = vec![0; length];
            let _ = reader.read_exact(&mut request);
            let _ = write!(
                stream,
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
}

#[test]
fn an_enterprise_server_answers_a_lookup_and_its_refusal_is_classified() {
    let server = crate::Server::enterprise("git.example.com").unwrap();
    let mut pr = same_repository();
    pr["url"] = "https://git.example.com/example/project/pull/8".into();
    let token = format!("ghes-found-{}", std::process::id());
    fake_server(&token, 200, numbered(pr).to_string());
    let token = crate::ServerToken::new(server.clone(), Arc::new(token.as_str().into()));
    let found = by_number("example", "project", 8, &token, || false, &mut None)
        .unwrap()
        .unwrap();
    assert_eq!(found.url, "https://git.example.com/example/project/pull/8");

    let refused = format!("ghes-refused-{}", std::process::id());
    fake_server(&refused, 401, r#"{"message":"Bad credentials"}"#.into());
    let refused = crate::ServerToken::new(server, Arc::new(refused.as_str().into()));
    assert!(matches!(
        by_number("example", "project", 8, &refused, || false, &mut None),
        Err(Error::Authentication)
    ));
}
