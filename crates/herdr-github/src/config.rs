//! `[github]`: the sign-in client, the plaintext opt-in, and the additional
//! and GitHub Enterprise Server accounts the GitHub page adds. An account's
//! token is never here: only the credential store entry it signs in under and
//! the owners it reads.

use crate::{Error, Result, Server, error::AccountsError};
use serde::Deserialize;
use std::env;

/// More accounts than anyone keeps apart by repository owner, additional and
/// GitHub Enterprise Server accounts together; the bound keeps every window's
/// per-account sign-in checks few.
pub const MAX_ACCOUNTS: usize = 8;
/// Owners one account lists besides its login and organizations.
const MAX_OWNERS: usize = 32;
const ID_BYTES: usize = 16;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct GitHubConfig {
    pub oauth_client_id: Option<String>,
    pub allow_plaintext_credentials: bool,
    /// Accounts read for the repositories they cover, in this order; any
    /// other repository is read with the main sign-in.
    pub accounts: Vec<GitHubAccount>,
    /// Accounts on GitHub Enterprise Servers. A table of its own, so a build
    /// that does not know it never reads them as GitHub.com accounts.
    pub enterprise: Vec<EnterpriseAccount>,
}

/// An additional GitHub account. `id` names its credential store entry, so it
/// stays the same when the account signs in again; `owners` are repository
/// owners it reads besides its own login and organizations. Fields a newer
/// build adds are ignored and named like any unknown key, and kept on edit.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct GitHubAccount {
    pub id: String,
    #[serde(default)]
    pub owners: Vec<String>,
}

impl GitHubAccount {
    /// A new account with a random ID and no owners of its own yet.
    pub fn generate() -> Result<Self> {
        Ok(Self {
            id: generate_id()?,
            owners: Vec::new(),
        })
    }
}

/// An account on the GitHub Enterprise Server `server`, a host name with an
/// optional HTTPS port (see [`Server::enterprise`]). `id` and `owners` are
/// as for [`GitHubAccount`]; the first account on a server also reads the
/// owners no other account there covers.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct EnterpriseAccount {
    pub server: String,
    pub id: String,
    #[serde(default)]
    pub owners: Vec<String>,
}

impl EnterpriseAccount {
    /// A new account on `server` with a random ID.
    pub fn generate(server: &Server) -> Result<Self> {
        Ok(Self {
            server: server.host().to_owned(),
            id: generate_id()?,
            owners: Vec::new(),
        })
    }

    /// The server, once [`GitHubConfig::check_accounts`] accepted it.
    pub fn server(&self) -> Option<Server> {
        Server::enterprise(&self.server)
    }
}

fn generate_id() -> Result<String> {
    let mut bytes = [0_u8; ID_BYTES];
    getrandom::fill(&mut bytes).map_err(AccountsError::Random)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// 32 lowercase hexadecimal digits: safe in a keyring account name and a
/// file name, which is where an account's ID ends up.
pub fn valid_account_id(id: &str) -> bool {
    id.len() == ID_BYTES * 2 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// A GitHub user or organization name, enterprise managed users' `_shortcode`
/// suffix included. GitHub stays the authority on the exact shape; this only
/// keeps the config to names that could be one.
pub fn valid_owner(owner: &str) -> bool {
    !owner.is_empty()
        && owner.len() <= 100
        && owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

/// Herdr GPUI's own GitHub App, used unless another client is configured.
const DEFAULT_CLIENT_ID: &str = "Iv23liurUcwxPjrdIFYT";

impl GitHubConfig {
    /// Where to install the App sign-in uses, known only for Herdr GPUI's
    /// own: another App's page cannot be found from its client ID.
    pub fn app_install_url(&self) -> Option<&'static str> {
        (self.client_id().ok().flatten().as_deref() == Some(DEFAULT_CLIENT_ID))
            .then_some("https://github.com/apps/herdr-ui/installations/new")
    }

    pub fn client_id(&self) -> Result<Option<String>> {
        self.client_id_with_override(env::var_os("HERDR_GITHUB_OAUTH_CLIENT_ID").as_deref())
    }

    pub fn client_id_with_override(
        &self,
        value: Option<&std::ffi::OsStr>,
    ) -> Result<Option<String>> {
        let (id, source) = match value {
            Some(value) => (
                Some(value.to_str().ok_or(Error::ClientIdEncoding)?),
                "HERDR_GITHUB_OAUTH_CLIENT_ID",
            ),
            None => (
                Some(self.oauth_client_id.as_deref().unwrap_or(DEFAULT_CLIENT_ID)),
                "github.oauth_client_id",
            ),
        };
        if let Some(id) = id
            && (id.is_empty()
                || id.len() > 256
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.')))
        {
            return Err(Error::InvalidClientId(source));
        }
        Ok(id.map(str::to_owned))
    }

    /// Keeps the accounts and owners this build holds, and names the rest as
    /// ignored keys: a newer build may allow more.
    pub fn limit_accounts(&mut self) -> Vec<String> {
        let mut ignored = limit("accounts", &mut self.accounts, MAX_ACCOUNTS, |account| {
            &mut account.owners
        });
        let room = MAX_ACCOUNTS - self.accounts.len();
        ignored.extend(limit("enterprise", &mut self.enterprise, room, |account| {
            &mut account.owners
        }));
        ignored
    }

    /// Refuses accounts the app could not tell apart or store.
    pub fn check_accounts(&self) -> Result<()> {
        check_accounts(&self.accounts, &self.enterprise).map_err(Error::from)
    }
}

fn limit<T>(
    table: &str,
    accounts: &mut Vec<T>,
    max: usize,
    owners: impl Fn(&mut T) -> &mut Vec<String>,
) -> Vec<String> {
    let mut ignored: Vec<String> = (max..accounts.len())
        .map(|index| format!("github.{table}.{index}"))
        .collect();
    accounts.truncate(max);
    for (index, account) in accounts.iter_mut().enumerate() {
        let owners = owners(account);
        ignored.extend(
            (MAX_OWNERS..owners.len())
                .map(|owner| format!("github.{table}.{index}.owners.{owner}")),
        );
        owners.truncate(MAX_OWNERS);
    }
    ignored
}

pub fn check_accounts(
    accounts: &[GitHubAccount],
    enterprise: &[EnterpriseAccount],
) -> std::result::Result<(), AccountsError> {
    let entries = accounts
        .iter()
        .map(|account| (&account.id, &account.owners))
        .chain(
            enterprise
                .iter()
                .map(|account| (&account.id, &account.owners)),
        );
    for (index, (id, owners)) in entries.clone().enumerate() {
        if !valid_account_id(id) {
            return Err(AccountsError::Id);
        }
        if entries.clone().take(index).any(|(other, _)| other == id) {
            return Err(AccountsError::DuplicateId(id.clone()));
        }
        if let Some(owner) = owners.iter().find(|owner| !valid_owner(owner)) {
            return Err(AccountsError::Owner(owner.chars().take(100).collect()));
        }
    }
    if let Some(account) = enterprise.iter().find(|account| account.server().is_none()) {
        return Err(AccountsError::Server(
            account.server.chars().take(100).collect(),
        ));
    }
    Ok(())
}
