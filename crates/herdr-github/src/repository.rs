//! GitHub repositories named by Git remote URLs, on GitHub.com or on a
//! configured GitHub Enterprise Server.

use crate::Server;

/// A bound on each remote path segment, above GitHub's own name limits.
const MAX_SEGMENT: usize = 100;

/// A repository and the server it lives on. The owner is lowercased.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repo {
    pub server: Server,
    pub owner: String,
    pub name: String,
}

impl Repo {
    /// Whether both name the same repository: owner and repository names
    /// are case-insensitive on every GitHub server.
    pub fn same(&self, other: &Self) -> bool {
        self.server == other.server
            && self.owner.eq_ignore_ascii_case(&other.owner)
            && self.name.eq_ignore_ascii_case(&other.name)
    }
}

/// One segment of a remote's path.
///
/// Account and repository names are GitHub's to define, so no name grammar is
/// reproduced here: a segment is refused only when it names the directory
/// itself or carries punctuation that would not stay inside its own path
/// segment in the avatar URL built from it.
fn segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SEGMENT
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// The repository a remote URL names on GitHub.com or on one of
/// `enterprise`. A remote on any other host is not a GitHub repository here,
/// so nothing is ever sent to a server no account signed in to.
pub fn github_repo(remote: &str, enterprise: &[Server]) -> Option<Repo> {
    let (server, path) = ssh_path(remote, enterprise).or_else(|| http_path(remote, enterprise))?;
    let (owner, repo) = path.split_once('/')?;
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    if !segment(owner) || !segment(repo) {
        return None;
    }
    Some(Repo {
        server,
        owner: owner.to_ascii_lowercase(),
        name: repo.to_owned(),
    })
}

/// The server and path of `git@host:path` or `ssh://git@host[:port]/path`.
/// An SSH port is not the server's HTTPS port, so only the name is compared.
/// GitHub.com takes SSH on its default port only.
fn ssh_path<'a>(remote: &'a str, enterprise: &[Server]) -> Option<(Server, &'a str)> {
    let (host, port, path) = match remote.strip_prefix("ssh://git@") {
        Some(rest) => {
            let (authority, path) = rest.split_once('/')?;
            match authority.split_once(':') {
                Some((host, _)) => (host, true, path),
                None => (authority, false, path),
            }
        }
        None => {
            let (host, path) = remote.strip_prefix("git@")?.split_once(':')?;
            (host, false, path)
        }
    };
    let server = known(enterprise, host, Server::name)?;
    (!port || server.is_enterprise()).then_some((server, path))
}

/// The server and path of an `http(s)://[userinfo@]host[:port]/` remote.
///
/// Git accepts a user name, or a user name and token, before the host
/// (`https://user@github.com/o/r`, `https://user:token@github.com/o/r`).
/// The host is what Git connects to, so it alone must name the server; the
/// userinfo is skipped and never kept. Userinfo that a URL parser could read
/// as the end of the authority (`/`, `?`, `#`, `\`, a second `@`) is refused
/// so the host cannot be misread.
fn http_path<'a>(remote: &'a str, enterprise: &[Server]) -> Option<(Server, &'a str)> {
    let rest = ["https://", "http://"]
        .into_iter()
        .find_map(|scheme| remote.strip_prefix(scheme))?;
    let (authority, path) = rest.split_once('/')?;
    let host = match authority.split_once('@') {
        None => authority,
        Some((userinfo, host)) => {
            let plain = |b: u8| b.is_ascii_graphic() && !matches!(b, b'@' | b'?' | b'#' | b'\\');
            if userinfo.is_empty() || !userinfo.bytes().all(plain) {
                return None;
            }
            host
        }
    };
    Some((known(enterprise, host, Server::host)?, path))
}

/// GitHub.com by its exact host, or the configured server whose `part`
/// (its host, or its name alone) is `host`.
fn known(enterprise: &[Server], host: &str, part: fn(&Server) -> &str) -> Option<Server> {
    if host == Server::GitHub.host() {
        return Some(Server::GitHub);
    }
    enterprise
        .iter()
        .find(|server| part(server).eq_ignore_ascii_case(host))
        .cloned()
}
