//! Which coding agents a host has installed.

use crate::{Error, Result};
use herdr_remote::teleport::{AgentKind, Host};
use std::sync::atomic::AtomicBool;

/// The agents installed on `host`, in [`AgentKind::ALL`] order.
pub(super) fn installed(host: Option<&Host>, cancelled: &AtomicBool) -> Result<Vec<AgentKind>> {
    let binaries = AgentKind::ALL.map(AgentKind::binary);
    let found = host
        .ok_or(Error::UnsupportedHost)?
        .installed_programs(&binaries, cancelled)
        .map_err(crate::error::script("Discovering installed agents"))?;
    Ok(agents(&found))
}

/// The agents whose binaries are among `found`.
pub(super) fn agents(found: &[String]) -> Vec<AgentKind> {
    AgentKind::ALL
        .into_iter()
        .filter(|kind| found.iter().any(|binary| binary == kind.binary()))
        .collect()
}
