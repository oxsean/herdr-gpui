#![allow(clippy::unwrap_used)]

use super::*;

fn profile(login: &str, token: &str, organizations: &[&str]) -> Profile {
    Profile::fixture(login, Arc::new(token.into()), organizations)
}

fn enterprise_profile(login: &str, token: &str, organizations: &[&str]) -> Profile {
    let mut profile = profile(login, token, organizations);
    profile.server = ghes();
    profile
}

fn ghes() -> Server {
    Server::enterprise("git.example.com").unwrap()
}

const GITHUB: &Server = &Server::GitHub;

fn exposed(token: Option<&ServerToken>) -> &str {
    token.map_or("", |token| {
        secrecy::ExposeSecret::expose_secret(&*token.token)
    })
}

#[test]
fn an_owner_no_additional_account_covers_is_read_with_the_main_account() {
    let main = profile("penso", "main-token", &[]);
    let emu = profile("fabien_microsoft", "emu-token", &["emu-org"]);
    let tokens = Tokens::new(Some(&main), [(&emu, &[][..])]);
    assert_eq!(exposed(tokens.for_owner(GITHUB, "rust-lang")), "main-token");
    assert_eq!(tokens.login_for(GITHUB, "rust-lang"), Some("penso"));
    // The main account's own repositories are the main account's.
    assert_eq!(exposed(tokens.for_owner(GITHUB, "penso")), "main-token");
}

#[test]
fn an_additional_account_reads_its_login_organizations_and_listed_owners() {
    let main = profile("penso", "main-token", &[]);
    let emu = profile("fabien_microsoft", "emu-token", &["emu-org"]);
    let owners = vec!["listed-owner".to_owned()];
    let tokens = Tokens::new(Some(&main), [(&emu, owners.as_slice())]);
    for owner in [
        "fabien_microsoft",
        "FABIEN_MICROSOFT",
        "emu-org",
        "Listed-Owner",
    ] {
        assert_eq!(
            exposed(tokens.for_owner(GITHUB, owner)),
            "emu-token",
            "{owner}"
        );
        assert_eq!(tokens.login_for(GITHUB, owner), Some("fabien_microsoft"));
    }
}

#[test]
fn the_first_listed_account_covering_an_owner_wins() {
    let main = profile("penso", "main-token", &[]);
    let first = profile("first", "first-token", &["shared"]);
    let second = profile("second", "second-token", &["shared"]);
    let tokens = Tokens::new(Some(&main), [(&first, &[][..]), (&second, &[][..])]);
    assert_eq!(exposed(tokens.for_owner(GITHUB, "shared")), "first-token");
    assert_eq!(exposed(tokens.for_owner(GITHUB, "second")), "second-token");
}

#[test]
fn an_owner_is_read_only_by_an_account_on_its_server() {
    let main = profile("penso", "main-token", &[]);
    let work = enterprise_profile("penso", "work-token", &[]);
    let tokens = Tokens::new(Some(&main), [(&work, &[][..])]);
    // The enterprise account's login covers that owner on its server only.
    assert_eq!(exposed(tokens.for_owner(GITHUB, "penso")), "main-token");
    assert_eq!(exposed(tokens.for_owner(&ghes(), "penso")), "work-token");
    // The first account on an enterprise server reads the owners no other
    // account there covers; GitHub.com's main sign-in never does.
    assert_eq!(exposed(tokens.for_owner(&ghes(), "acme")), "work-token");
    let other = Server::enterprise("other.example.com").unwrap();
    assert!(tokens.for_owner(&other, "penso").is_none());
    assert_eq!(tokens.enterprise(), [ghes()]);
    // The same login on two servers is two accounts.
    assert_eq!(tokens.name_for(GITHUB, "penso").unwrap(), "penso");
    assert_eq!(
        tokens.name_for(&ghes(), "penso").unwrap(),
        "penso@git.example.com"
    );
}

#[test]
fn without_a_main_sign_in_only_covered_owners_are_read() {
    let emu = profile("emu", "emu-token", &["emu-org"]);
    let work = enterprise_profile("dev", "work-token", &[]);
    let tokens = Tokens::new(None, [(&emu, &[][..]), (&work, &[][..])]);
    assert!(tokens.main().is_none());
    assert_eq!(exposed(tokens.for_owner(GITHUB, "emu-org")), "emu-token");
    assert!(tokens.for_owner(GITHUB, "rust-lang").is_none());
    assert_eq!(exposed(tokens.for_owner(&ghes(), "anyone")), "work-token");
    assert_eq!(
        tokens.logins().collect::<Vec<_>>(),
        ["emu", "dev@git.example.com"]
    );
}

#[test]
fn sameness_follows_token_allocations_and_coverage_not_token_text() {
    let main = profile("penso", "main-token", &[]);
    let emu = profile("emu", "emu-token", &[]);
    let tokens = Tokens::new(Some(&main), [(&emu, &[][..])]);
    assert!(tokens.same(&tokens.clone()));
    assert!(tokens.same(&Tokens::new(Some(&main), [(&emu, &[][..])])));
    // A renewed token is a new allocation, even with the same text.
    let renewed = profile("emu", "emu-token", &[]);
    assert!(!tokens.same(&Tokens::new(Some(&main), [(&renewed, &[][..])])));
    let owners = vec!["more".to_owned()];
    assert!(!tokens.same(&Tokens::new(Some(&main), [(&emu, owners.as_slice())])));
    assert!(!tokens.same(&Tokens::new(Some(&main), [])));
    let moved = enterprise_profile("emu", "emu-token", &[]);
    assert!(!tokens.same(&Tokens::new(Some(&main), [(&moved, &[][..])])));
}

#[test]
fn the_key_names_every_account_main_first_with_the_owners_it_reads() {
    let main = profile("penso", "main-token", &[]);
    let emu = profile("emu", "emu-token", &["emu-org"]);
    assert_eq!(
        Tokens::new(Some(&main), [(&emu, &[][..])]).key(),
        "penso,emu(emu emu-org)"
    );
    assert_eq!(Tokens::new(Some(&main), []).key(), "penso");
    // Moving an owner to an account is another set, whose answers are its own.
    let owners = vec!["acme".to_owned()];
    assert_eq!(
        Tokens::new(Some(&main), [(&emu, owners.as_slice())]).key(),
        "penso,emu(emu emu-org acme)"
    );
    let work = enterprise_profile("dev", "work-token", &[]);
    assert_eq!(
        Tokens::new(Some(&main), [(&work, &[][..])]).key(),
        "penso,dev@git.example.com(dev)"
    );
    let tokens = Tokens::new(Some(&main), [(&emu, &[][..])]);
    assert_eq!(tokens.logins().collect::<Vec<_>>(), ["penso", "emu"]);
}

#[test]
fn a_lone_token_reads_every_owner_on_its_server() {
    let token: Arc<SecretString> = Arc::new("lone".into());
    let tokens = Tokens::from(token.clone());
    let read = tokens.for_owner(GITHUB, "anyone").unwrap();
    assert!(Arc::ptr_eq(&read.token, &token));
    assert!(tokens.for_owner(&ghes(), "anyone").is_none());
}
