use super::*;

#[test]
fn upstream_matches_renamed_fork_branch_and_rejects_unrelated_heads() {
    let head = super::super::fetch::upstream_head("pr/138", &crate::Server::GitHub, &[], |key| {
        Ok(match key {
            "branch.pr/138.remote" => Some("contributor".into()),
            "branch.pr/138.merge" => Some("refs/heads/feat/inline-ime-preedit".into()),
            "remote.contributor.url" => Some("git@github.com:renga-kogahara/herdr-gpui.git".into()),
            _ => panic!("unexpected config read: {key}"),
        })
    })
    .unwrap()
    .unwrap();
    assert_eq!(head.branch, "feat/inline-ime-preedit");
    let mut pr = response()[0].clone();
    pr["headRefName"] = head.branch.clone().into();
    pr["headRepositoryOwner"] = serde_json::json!({"login":"renga-kogahara"});
    pr["headRepository"] = serde_json::json!({"name":"herdr-gpui"});
    let graphql =
        |nodes| serde_json::json!({"data":{"repository":{"pullRequests":{"nodes":nodes}}}});
    let parse = |nodes| {
        parse_graphql(
            graphql(nodes),
            &crate::Server::GitHub,
            "example",
            "project",
            &head.branch,
            Some(&head),
        )
    };
    let mut stranger = pr.clone();
    stranger["headRepositoryOwner"] = serde_json::json!({"login":"stranger"});
    assert!(parse(serde_json::json!([stranger])).unwrap().is_none());
    assert_eq!(
        parse(serde_json::json!([stranger, pr]))
            .unwrap()
            .unwrap()
            .number,
        8
    );
    let mut wrong_repo = pr.clone();
    wrong_repo["headRepository"] = serde_json::json!({"name":"different-fork"});
    assert!(parse(serde_json::json!([wrong_repo])).unwrap().is_none());
    let mut wrong_branch = pr.clone();
    wrong_branch["headRefName"] = "pr/138".into();
    assert!(parse(serde_json::json!([wrong_branch])).unwrap().is_none());
    assert!(matches!(
        parse(serde_json::json!([pr, pr])),
        Err(Error::PullRequest(crate::pull_request::Error::Ambiguous))
    ));
    let mut truncated = graphql(serde_json::json!([pr]));
    truncated["data"]["repository"]["pullRequests"]["pageInfo"] =
        serde_json::json!({"hasNextPage":true});
    assert!(matches!(
        parse_graphql(
            truncated,
            &crate::Server::GitHub,
            "example",
            "project",
            &head.branch,
            Some(&head)
        ),
        Err(Error::PullRequest(crate::pull_request::Error::Ambiguous))
    ));
}

#[test]
fn no_upstream_preserves_origin_owner_and_local_branch_matching() {
    let head =
        super::super::fetch::upstream_head("feature", &crate::Server::GitHub, &[], |_| Ok(None))
            .unwrap();
    assert!(head.is_none());
    let graphql =
        |nodes| serde_json::json!({"data":{"repository":{"pullRequests":{"nodes":nodes}}}});
    assert!(
        parse_graphql(
            graphql(response()),
            &crate::Server::GitHub,
            "example",
            "project",
            "feature",
            head.as_ref()
        )
        .unwrap()
        .is_some()
    );
    let mut fork = response();
    fork[0]["headRepositoryOwner"] = serde_json::json!({"login":"stranger"});
    assert!(matches!(
        parse_graphql(
            graphql(fork),
            &crate::Server::GitHub,
            "example",
            "project",
            "feature",
            None
        ),
        Err(Error::PullRequest(crate::pull_request::Error::Identity))
    ));
    assert!(matches!(
        parse_graphql(
            graphql(response()),
            &crate::Server::GitHub,
            "example",
            "project",
            "pr/138",
            None
        ),
        Err(Error::PullRequest(crate::pull_request::Error::Identity))
    ));
}

#[test]
fn unsupported_upstream_and_config_errors_do_not_fall_back() {
    for merge in ["refs/heads/feature", "refs/pull/138/head"] {
        assert!(
            super::super::fetch::upstream_head("feature", &crate::Server::GitHub, &[], |key| Ok(
                Some(
                    match key {
                        "branch.feature.remote" => "fork",
                        "branch.feature.merge" => merge,
                        "remote.fork.url" => "https://other.example/owner/repo.git",
                        _ => panic!("unexpected key"),
                    }
                    .into()
                )
            ))
            .is_err()
        );
    }
    assert!(matches!(
        super::super::fetch::upstream_head("feature", &crate::Server::GitHub, &[], |_| Err(
            crate::pull_request::Error::Cancelled.into()
        )),
        Err(Error::PullRequest(crate::pull_request::Error::Cancelled))
    ));
}

#[test]
fn an_upstream_on_another_server_is_refused() {
    let ghes = crate::Server::enterprise("git.example.com").unwrap();
    let config = |url: &'static str| {
        move |key: &str| {
            Ok(Some(
                match key {
                    "branch.feature.remote" => "fork",
                    "branch.feature.merge" => "refs/heads/feature",
                    "remote.fork.url" => url,
                    _ => panic!("unexpected key"),
                }
                .into(),
            ))
        }
    };
    let enterprise = [ghes.clone()];
    let head = super::super::fetch::upstream_head(
        "feature",
        &ghes,
        &enterprise,
        config("git@git.example.com:fork/project.git"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        (head.owner.as_str(), head.repo.as_str()),
        ("fork", "project")
    );
    assert!(matches!(
        super::super::fetch::upstream_head(
            "feature",
            &ghes,
            &enterprise,
            config("git@github.com:fork/project.git"),
        ),
        Err(Error::PullRequest(crate::pull_request::Error::Origin))
    ));
}
