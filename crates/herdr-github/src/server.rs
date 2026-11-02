//! The GitHub server an account signs in to and a repository lives on:
//! GitHub.com, or a GitHub Enterprise Server host from the config. A token is
//! sent only to the API of the server its account signed in to.

use std::{fmt, sync::Arc};

/// A bound above the longest DNS name, with room for a port.
const MAX_HOST: usize = 260;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Server {
    #[default]
    GitHub,
    /// A GitHub Enterprise Server host name, lowercased, with an HTTPS port
    /// when it is not 443.
    Enterprise(Arc<str>),
}

impl Server {
    /// A GitHub Enterprise Server as the config names it: a host name and an
    /// optional HTTPS port, without a scheme or path. GitHub.com is refused,
    /// because its accounts sign in on their own.
    pub fn enterprise(host: &str) -> Option<Self> {
        let host = host.to_ascii_lowercase();
        let (name, port) = match host.split_once(':') {
            Some((name, port)) => (name, Some(port)),
            None => (host.as_str(), None),
        };
        let label = |label: &str| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        };
        let port_ok = port.is_none_or(|port| {
            !port.starts_with('0') && port.parse::<u16>().is_ok_and(|port| port != 0)
        });
        if host.len() > MAX_HOST
            || !name.split('.').all(label)
            || !port_ok
            || matches!(name, "github.com" | "www.github.com" | "api.github.com")
        {
            return None;
        }
        let host = match port {
            Some("443") => name.to_owned(),
            _ => host.clone(),
        };
        Some(Self::Enterprise(host.into()))
    }

    /// The host and HTTPS port the server's web pages and API are on.
    pub fn host(&self) -> &str {
        match self {
            Self::GitHub => "github.com",
            Self::Enterprise(host) => host,
        }
    }

    /// The host name alone, which is all an SSH remote can name: its port is
    /// the SSH port, never the server's HTTPS port.
    pub(crate) fn name(&self) -> &str {
        self.host().split(':').next().unwrap_or_default()
    }

    /// The name the account strip shows: the organization part of the host,
    /// such as `acme` for `ghe.acme.com`, or the host when another of
    /// `servers` has the same part.
    pub fn short_name<'s>(&self, servers: impl IntoIterator<Item = &'s Self>) -> &str {
        let short = self.organization();
        let shared = servers
            .into_iter()
            .any(|other| other != self && other.organization() == short);
        if shared { self.host() } else { short }
    }

    fn organization(&self) -> &str {
        let name = self.name();
        let labels: Vec<&str> = name.split('.').collect();
        if labels
            .iter()
            .all(|label| label.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return name;
        }
        // Under a country domain such as `co.uk` the organization sits one label further in.
        let suffix = match labels.as_slice() {
            [.., second, last]
                if last.len() == 2
                    && matches!(*second, "ac" | "co" | "com" | "edu" | "gov" | "net" | "org") =>
            {
                2
            }
            _ => 1,
        };
        labels
            .len()
            .checked_sub(suffix + 1)
            .map_or(name, |index| labels[index])
    }

    /// The server a remote's host names: GitHub.com, or one of `enterprise`
    /// by its host, or by its name alone, as an SSH remote names it.
    pub fn of_host(host: &str, enterprise: &[Self]) -> Option<Self> {
        if host.eq_ignore_ascii_case("github.com") {
            return Some(Self::GitHub);
        }
        enterprise
            .iter()
            .find(|server| {
                server.host().eq_ignore_ascii_case(host) || server.name().eq_ignore_ascii_case(host)
            })
            .cloned()
    }

    pub fn is_enterprise(&self) -> bool {
        matches!(self, Self::Enterprise(_))
    }

    /// The server's web address, such as `https://github.com`.
    pub fn web_url(&self) -> String {
        format!("https://{}", self.host())
    }

    pub(crate) fn graphql_url(&self) -> String {
        match self {
            Self::GitHub => "https://api.github.com/graphql".to_owned(),
            Self::Enterprise(host) => format!("https://{host}/api/graphql"),
        }
    }

    /// The REST address of `path`, such as `/user`, under the server's API root.
    pub(crate) fn rest_url(&self, path: &str) -> String {
        match self {
            Self::GitHub => format!("https://api.github.com{path}"),
            Self::Enterprise(host) => format!("https://{host}/api/v3{path}"),
        }
    }

    /// The name an account on this server is known by in logs and pauses:
    /// its login on GitHub.com, `login@host` elsewhere, so the same login on
    /// two servers stays two accounts.
    pub fn account_name(&self, login: &str) -> String {
        match self {
            Self::GitHub => login.to_owned(),
            Self::Enterprise(host) => format!("{login}@{host}"),
        }
    }
}

impl fmt::Display for Server {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.host())
    }
}

#[cfg(test)]
mod tests;
