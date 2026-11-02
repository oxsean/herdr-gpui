//! Writes a GraphQL document the crate sends to `HERDR_GRAPHQL_EXPORT`, so a
//! GitHub Enterprise compatible server can check its schema against it.

#![allow(clippy::expect_used)]

pub(crate) fn export(name: &str, document: &str) {
    if let Some(dir) = std::env::var_os("HERDR_GRAPHQL_EXPORT") {
        let path = std::path::Path::new(&dir).join(format!("{name}.graphql"));
        std::fs::write(path, format!("{}\n", document.trim())).expect("write GraphQL document");
    }
}
