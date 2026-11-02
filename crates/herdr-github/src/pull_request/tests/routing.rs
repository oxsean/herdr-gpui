use super::*;
use crate::{
    Profile, Server, Tokens,
    pull_request::{
        batch::{Batch, by_token},
        fetch::{Ask, Target},
    },
};
use secrecy::SecretString;

fn profile(login: &str, token: &Arc<SecretString>) -> Profile {
    Profile::fixture(login, token.clone(), &[])
}

fn target(owner: &str, branch: &str) -> (Input, Target) {
    (
        Input {
            checkout: None,
            repo_key: Some(format!("/{owner}/.git")),
            branch: branch.into(),
        },
        Target {
            server: Server::GitHub,
            owner: owner.into(),
            repo: "project".into(),
            ask: Ask::Number(1),
        },
    )
}

#[test]
fn each_account_asks_only_for_the_repositories_it_reads() {
    let main: Arc<SecretString> = Arc::new("main".into());
    let emu: Arc<SecretString> = Arc::new("emu".into());
    let (main_profile, emu_profile) = (profile("penso", &main), profile("fabien_microsoft", &emu));
    let tokens = Tokens::new(Some(&main_profile), [(&emu_profile, &[][..])]);
    let groups = by_token(
        vec![
            target("rust-lang", "a"),
            target("fabien_microsoft", "b"),
            target("penso", "c"),
            target("Fabien_Microsoft", "d"),
        ],
        &tokens,
        &mut Batch::default(),
    );
    let shape: Vec<(bool, Vec<&str>)> = groups
        .iter()
        .map(|(_, token, targets)| {
            (
                Arc::ptr_eq(&token.token, &main),
                targets
                    .iter()
                    .map(|(input, _)| input.branch.as_str())
                    .collect(),
            )
        })
        .collect();
    // In the order each account's first repository came.
    assert_eq!(shape, [(true, vec!["a", "c"]), (false, vec!["b", "d"])]);
    assert!(Arc::ptr_eq(&groups[1].1.token, &emu));
    // Each group names the account it is, which a pause is kept under.
    assert_eq!(groups[0].0, "penso");
    assert_eq!(groups[1].0, "fabien_microsoft");
}

#[test]
fn a_lone_token_asks_for_everything_in_one_group() {
    let token: Arc<SecretString> = Arc::new("lone".into());
    let groups = by_token(
        vec![target("one", "a"), target("two", "b")],
        &Tokens::from(token.clone()),
        &mut Batch::default(),
    );
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].2.len(), 2);
}

#[test]
fn a_repository_no_account_on_its_server_reads_fails_alone() {
    let work: Arc<SecretString> = Arc::new("work".into());
    let ghes = Server::enterprise("git.example.com").unwrap();
    let mut work_profile = profile("penso", &work);
    work_profile.server = ghes.clone();
    // No GitHub.com main sign-in: a GitHub.com repository is not read with
    // the enterprise token, even under the same login.
    let tokens = Tokens::new(None, [(&work_profile, &[][..])]);
    let mut on_ghes = target("acme", "b");
    on_ghes.1.server = ghes;
    let mut batch = Batch::default();
    let groups = by_token(vec![target("penso", "a"), on_ghes], &tokens, &mut batch);
    assert_eq!(groups.len(), 1);
    assert!(Arc::ptr_eq(&groups[0].1.token, &work));
    assert_eq!(groups[0].0, "penso@git.example.com");
    assert_eq!(groups[0].2[0].0.branch, "b");
    assert!(matches!(
        batch.found.as_slice(),
        [(input, Err(Error::Authentication))] if input.branch == "a"
    ));
}

#[test]
fn adding_an_account_rescopes_the_cache_and_keeps_its_answers_apart() {
    let main: Arc<SecretString> = Arc::new("main".into());
    let emu: Arc<SecretString> = Arc::new("emu".into());
    let (main_profile, emu_profile) = (profile("penso", &main), profile("emu", &emu));
    let mut cache = Cache::default();
    let alone = Tokens::new(Some(&main_profile), []);
    let key = alone.key();
    cache.scope((0, 1, "boot".into()), alone, &key, Origin::Local);
    assert_eq!(cache.view.login, "penso");
    let both = Tokens::new(Some(&main_profile), [(&emu_profile, &[][..])]);
    let key = both.key();
    cache.scope((0, 1, "boot".into()), both.clone(), &key, Origin::Local);
    assert_eq!(cache.view.login, "penso,emu(emu)");
    assert!(cache.token.as_ref().unwrap().same(&both));
}
