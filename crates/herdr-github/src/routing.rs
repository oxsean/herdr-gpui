//! Which signed-in account reads a repository. Accounts are chosen on the
//! repository's server first, never across servers: on GitHub.com an
//! additional account when the owner is its login, one of its organizations,
//! or an owner its config lists, otherwise the main sign-in; on a GitHub
//! Enterprise Server the same rule over that server's accounts, the first
//! one standing in for a main sign-in. Workers learn a checkout's repository
//! from its remote, so they carry every token and choose once they know it.

use super::{Profile, Server};
use secrecy::SecretString;
use std::sync::Arc;

/// A token and the server its account signed in to. Requests made with it
/// go to that server's API only.
#[derive(Clone)]
pub struct ServerToken {
    pub server: Server,
    pub token: Arc<SecretString>,
}

impl ServerToken {
    pub fn new(server: Server, token: Arc<SecretString>) -> Self {
        Self { server, token }
    }

    /// A GitHub.com token.
    pub fn github(token: Arc<SecretString>) -> Self {
        Self::new(Server::GitHub, token)
    }
}

/// One account's token and the owners it reads. The main sign-in's list is
/// empty because it reads every GitHub.com owner no other account claims.
struct Route {
    login: String,
    token: ServerToken,
    owners: Vec<String>,
    main: bool,
}

impl Route {
    fn covers(&self, owner: &str) -> bool {
        self.owners
            .iter()
            .any(|covered| covered.eq_ignore_ascii_case(owner))
    }

    fn name(&self) -> String {
        self.token.server.account_name(&self.login)
    }
}

/// The tokens a window reads GitHub with. Cheap to clone.
#[derive(Clone)]
pub struct Tokens(Arc<[Route]>);

impl Tokens {
    /// `main` for every GitHub.com owner, except those an additional account
    /// on GitHub.com covers: its login, its organizations, and `owners` from
    /// its config. The first listed account covering an owner wins. Without
    /// a main sign-in, GitHub.com owners no account covers are not read.
    pub fn new<'a>(
        main: Option<&Profile>,
        additional: impl IntoIterator<Item = (&'a Profile, &'a [String])>,
    ) -> Self {
        let main = main.map(|main| Route {
            login: main.login.clone(),
            token: ServerToken::new(main.server.clone(), main.token.clone()),
            owners: Vec::new(),
            main: true,
        });
        let routes = main
            .into_iter()
            .chain(additional.into_iter().map(|(profile, owners)| {
                Route {
                    login: profile.login.clone(),
                    token: ServerToken::new(profile.server.clone(), profile.token.clone()),
                    owners: std::iter::once(&profile.login)
                        .chain(profile.organizations.iter().flatten())
                        .chain(owners)
                        .cloned()
                        .collect(),
                    main: false,
                }
            }));
        Self(routes.collect())
    }

    fn route(&self, server: &Server, owner: &str) -> Option<&Route> {
        let mut on_server = self.0.iter().filter(|route| &route.token.server == server);
        on_server
            .clone()
            .find(|route| !route.main && route.covers(owner))
            .or_else(|| on_server.find(|route| route.main || server.is_enterprise()))
    }

    /// The token that reads repositories owned by `owner` on `server`, if
    /// any account there does.
    pub fn for_owner(&self, server: &Server, owner: &str) -> Option<&ServerToken> {
        self.route(server, owner).map(|route| &route.token)
    }

    /// The account that reads repositories owned by `owner` on `server`, by
    /// its name (see [`Server::account_name`]).
    pub fn name_for(&self, server: &Server, owner: &str) -> Option<String> {
        self.route(server, owner).map(Route::name)
    }

    /// The login of the account that reads repositories owned by `owner`
    /// on `server`.
    pub fn login_for(&self, server: &Server, owner: &str) -> Option<&str> {
        self.route(server, owner).map(|route| route.login.as_str())
    }

    /// The main sign-in's token, when there is one.
    pub fn main(&self) -> Option<&ServerToken> {
        self.0
            .iter()
            .find(|route| route.main)
            .map(|route| &route.token)
    }

    /// The GitHub Enterprise Servers some account signed in to: the only
    /// servers besides GitHub.com whose remotes are read as repositories.
    pub fn enterprise(&self) -> Vec<Server> {
        let mut servers: Vec<Server> = Vec::new();
        for route in self.0.iter() {
            let server = &route.token.server;
            if server.is_enterprise() && !servers.contains(server) {
                servers.push(server.clone());
            }
        }
        servers
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The same accounts, owners, and token allocations. A renewed token is a
    /// new allocation, which is how caches notice it, never by its text.
    pub fn same(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self.0.iter().zip(other.0.iter()).all(|(this, that)| {
                Arc::ptr_eq(&this.token.token, &that.token.token)
                    && this.token.server == that.token.server
                    && this.login == that.login
                    && this.owners == that.owners
                    && this.main == that.main
            })
    }

    /// Every account's name, main first, each additional one with the
    /// owners it reads: names the account set answers belong to. Moving an
    /// owner to another account is a new set, so no answer or hidden
    /// repository the old account recorded is shown for the new one.
    pub fn key(&self) -> String {
        let routes: Vec<String> = self
            .0
            .iter()
            .map(|route| match route.owners.as_slice() {
                [] => route.name(),
                owners => format!("{}({})", route.name(), owners.join(" ")),
            })
            .collect();
        routes.join(",")
    }

    /// Every account's name, main first (see [`Server::account_name`]).
    pub fn logins(&self) -> impl Iterator<Item = String> + '_ {
        self.0.iter().map(Route::name)
    }
}

/// One GitHub.com token read as every GitHub.com owner, as before additional
/// accounts.
impl From<Arc<SecretString>> for Tokens {
    fn from(token: Arc<SecretString>) -> Self {
        ServerToken::github(token).into()
    }
}

/// One token read as every owner on its server.
impl From<ServerToken> for Tokens {
    fn from(token: ServerToken) -> Self {
        Self(Arc::new([Route {
            login: String::new(),
            token,
            owners: Vec::new(),
            main: true,
        }]))
    }
}

#[cfg(test)]
mod tests;
