//! Adding a GitHub Enterprise Server account on the GitHub page: its server
//! first, then the pasted token it signs in with. Device codes, the GitHub
//! App, and the GitHub CLI exist on GitHub.com only, so none is offered.

use crate::i18n::fl;
use crate::{
    HerdrWindow,
    config::{Config, EnterpriseAccount},
    search_input::SearchInput,
};
use gpui::{prelude::*, *};
use herdr_github::Server;

impl HerdrWindow {
    /// Whether another account fits beside the configured ones.
    pub(super) fn can_add_enterprise_account(&self) -> bool {
        self.menu.github_accounts.len() < crate::config::MAX_ACCOUNTS
    }

    pub(super) fn open_github_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(SearchInput::new);
        input.update(cx, |input, cx| {
            input.set_placeholder_with(|| fl!("github-menu-server-placeholder"), cx);
            input.set_appearance(self.config.ui.clone(), self.theme.clone(), cx);
            window.focus(&input.focus, cx);
        });
        self.close_github_token();
        self.menu.error = None;
        self.menu.github_server = Some(input);
    }

    pub(super) fn close_github_server(&mut self) {
        self.menu.github_server = None;
    }

    /// Add an account on the typed server and show it, so a token can be
    /// pasted for it. The config edit runs off the UI thread.
    pub(super) fn submit_github_server(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(input) = self.menu.github_server.as_ref() else {
            return;
        };
        let text = input.read(cx).text().trim().to_owned();
        if text.is_empty() {
            return;
        }
        let Some(server) = Server::enterprise(&text) else {
            self.menu.error = Some(fl!("github-menu-server-invalid"));
            return;
        };
        if !self.can_add_enterprise_account() {
            self.menu.error = Some(fl!(
                "github-menu-too-many-accounts",
                count = crate::config::MAX_ACCOUNTS
            ));
            return;
        }
        let account = match EnterpriseAccount::generate(&server) {
            Ok(account) => account,
            Err(error) => {
                self.menu.error = Some(error.to_string());
                return;
            }
        };
        tracing::info!(
            category = "github_accounts",
            "Adding a GitHub Enterprise Server account"
        );
        self.close_github_server();
        window.focus(&self.menu.focus, cx);
        self.menu.error = None;
        self.menu.github_adding = Some(account.id.clone());
        self.write_preference(move || Config::add_enterprise_account(&account), cx);
    }

    /// The server field and what it takes, while it is open.
    pub(super) fn render_github_server(&self) -> Option<Div> {
        let input = self.menu.github_server.clone()?;
        Some(
            div()
                .debug_selector(|| "github-server".into())
                .mt(px(12.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(fl!("github-menu-server-label"))
                .child(input)
                .child(
                    div()
                        .text_color(rgb(self.theme.muted))
                        .child(fl!("github-menu-server-help")),
                ),
        )
    }
}
