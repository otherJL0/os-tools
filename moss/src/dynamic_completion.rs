use clap_complete::CompletionCandidate;
use std::env;
use std::path::PathBuf;

use super::{Installation, client::Client, package};

fn generate_results(client: Client, flags: package::Flags, prefix: &str) -> Vec<CompletionCandidate> {
    client
        .prefix_search(prefix, flags)
        .map(|name| CompletionCandidate::from(name.as_str()))
        .collect()
}

fn client(client_name: &str) -> Client {
    let root = PathBuf::from(env::var("MOSS_ROOT").unwrap_or_else(|_| String::from("/")));
    let installation = Installation::open(root, None).unwrap();
    Client::new(client_name, installation).unwrap()
}

pub fn prefix_completer(
    client_name: &str,
    flags: package::Flags,
) -> impl Fn(&std::ffi::OsStr) -> Vec<CompletionCandidate> {
    move |prefix: &std::ffi::OsStr| {
        let Some(prefix) = prefix.to_str() else {
            return vec![];
        };
        if prefix.is_empty() {
            return vec![];
        }
        generate_results(client(client_name), flags, prefix)
    }
}
