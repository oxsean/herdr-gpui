#![allow(clippy::unwrap_used)]

use super::*;

#[test]
fn enterprise_hosts_are_host_names_with_an_optional_https_port() {
    for (host, expected) in [
        ("ghe.example.com", "ghe.example.com"),
        ("GHE.Example.com", "ghe.example.com"),
        ("ghe.example.com:8443", "ghe.example.com:8443"),
        ("ghe.example.com:443", "ghe.example.com"),
        ("localhost", "localhost"),
    ] {
        assert_eq!(
            Server::enterprise(host).as_ref().map(Server::host),
            Some(expected),
            "{host}"
        );
    }
    for host in [
        "",
        "github.com",
        "GitHub.com",
        "api.github.com",
        "https://ghe.example.com",
        "ghe.example.com/api",
        "ghe.example.com:",
        "ghe.example.com:0",
        "ghe.example.com:65536",
        "ghe.example.com:080",
        "-ghe.example.com",
        "ghe..example.com",
        "user@ghe.example.com",
    ] {
        assert_eq!(Server::enterprise(host), None, "{host}");
    }
}

#[test]
fn each_server_has_its_own_api_and_web_addresses() {
    let github = Server::GitHub;
    assert_eq!(github.graphql_url(), "https://api.github.com/graphql");
    assert_eq!(github.rest_url("/user"), "https://api.github.com/user");
    assert_eq!(github.web_url(), "https://github.com");
    let enterprise = Server::enterprise("ghe.example.com:8443").unwrap();
    assert_eq!(
        enterprise.graphql_url(),
        "https://ghe.example.com:8443/api/graphql"
    );
    assert_eq!(
        enterprise.rest_url("/user"),
        "https://ghe.example.com:8443/api/v3/user"
    );
    assert_eq!(enterprise.web_url(), "https://ghe.example.com:8443");
    assert_eq!(enterprise.name(), "ghe.example.com");
}

#[test]
fn the_same_login_on_two_servers_is_two_account_names() {
    let enterprise = Server::enterprise("ghe.example.com").unwrap();
    assert_eq!(Server::GitHub.account_name("octo"), "octo");
    assert_eq!(enterprise.account_name("octo"), "octo@ghe.example.com");
}

#[test]
fn a_remote_host_names_only_github_or_a_signed_in_server() {
    let enterprise = [Server::enterprise("ghe.example.com:8443").unwrap()];
    assert_eq!(Server::of_host("github.com", &[]), Some(Server::GitHub));
    for host in ["ghe.example.com:8443", "GHE.example.com"] {
        assert_eq!(
            Server::of_host(host, &enterprise),
            Some(enterprise[0].clone()),
            "{host}"
        );
    }
    assert_eq!(Server::of_host("ghe.example.com", &[]), None);
    assert_eq!(Server::of_host("other.example.com", &enterprise), None);
}

#[test]
fn remotes_name_github_com_or_a_configured_server_and_nothing_else() {
    let plain = Server::enterprise("ghe.example.com").unwrap();
    let ported = Server::enterprise("git.example.com:8443").unwrap();
    let enterprise = [plain.clone(), ported.clone()];
    let repo = |remote| {
        crate::github_repo(remote, &enterprise).map(|repo| (repo.server, repo.owner, repo.name))
    };
    let named = |server: &Server| Some((server.clone(), "team".to_owned(), "repo".to_owned()));
    for (remote, server) in [
        ("https://GHE.example.com/Team/repo.git", &plain),
        ("https://user:token@ghe.example.com/team/repo", &plain),
        ("git@ghe.example.com:team/repo.git", &plain),
        // An SSH port is the SSH server's, whatever the HTTPS port.
        ("ssh://git@ghe.example.com:2222/team/repo", &plain),
        ("ssh://git@git.example.com/team/repo", &ported),
        ("https://git.example.com:8443/team/repo", &ported),
        ("git@github.com:team/repo", &Server::GitHub),
    ] {
        assert_eq!(repo(remote), named(server), "{remote}");
    }
    for remote in [
        "https://git.example.com/team/repo",
        "https://ghe.example.com:8443/team/repo",
        "https://other.example.com/team/repo",
        "https://ghe.example.com@evil.test/team/repo",
        "https://ghe.example.com.evil.test/team/repo",
        // GitHub.com stays as strict as it is without configured servers.
        "ssh://git@github.com:22/team/repo",
        "https://github.com:443/team/repo",
    ] {
        assert_eq!(repo(remote), None, "{remote}");
    }
    assert_eq!(
        crate::github_repo("git@ghe.example.com:team/repo", &[]),
        None
    );
}

#[test]
fn an_enterprise_server_is_verified_with_the_system_certificates() {
    let roots = |server: &Server| {
        crate::http::agent(server, std::time::Duration::from_secs(1))
            .config()
            .tls_config()
            .root_certs()
            .clone()
    };
    let enterprise = Server::enterprise("ghe.example.com").unwrap();
    assert!(matches!(
        roots(&enterprise),
        ureq::tls::RootCerts::PlatformVerifier
    ));
    assert!(matches!(
        roots(&Server::GitHub),
        ureq::tls::RootCerts::WebPki
    ));
}

#[test]
fn short_names_keep_the_organization_unless_two_servers_share_it() {
    let server = |host: &str| Server::enterprise(host).unwrap();
    for (host, expected) in [
        ("git.cooclue.cn", "cooclue"),
        ("cooclue.cn:8443", "cooclue"),
        ("ghe.acme.co.uk", "acme"),
        ("git.acme.com.cn", "acme"),
        ("localhost", "localhost"),
        ("ghe", "ghe"),
        ("10.0.0.5", "10.0.0.5"),
    ] {
        assert_eq!(server(host).short_name([]), expected, "{host}");
    }
    let servers = [
        server("git.acme.com"),
        server("ghe.acme.com"),
        server("ghe.other.com"),
    ];
    assert_eq!(servers[0].short_name(&servers), "git.acme.com");
    assert_eq!(servers[1].short_name(&servers), "ghe.acme.com");
    assert_eq!(servers[2].short_name(&servers), "other");
    assert_eq!(servers[0].short_name([&servers[0]]), "acme");
}
