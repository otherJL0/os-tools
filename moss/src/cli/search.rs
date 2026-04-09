// SPDX-FileCopyrightText: 2024 AerynOS Developers
// SPDX-License-Identifier: MPL-2.0

use std::collections::BTreeMap;

use clap::builder::NonEmptyStringValueParser;
use clap::{Arg, ArgMatches, Command};

use moss::client;
use moss::dependency;
use moss::package::{self, Name};
use moss::{Client, Installation, Provider, environment};
use strum::Display;
use tui::Styled;
use tui::pretty::{ColumnDisplay, print_columns};

const ARG_KEYWORD: &str = "KEYWORD";
const FLAG_INSTALLED: &str = "installed";
const FLAG_PROVIDES: &str = "provides";

/// Returns the Clap struct for this command.
pub fn command() -> Command {
    Command::new("search")
        .visible_alias("sr")
        .about("Search packages")
        .long_about("Search packages by looking into package names and summaries.")
        .arg(
            Arg::new(ARG_KEYWORD)
                .required(true)
                .num_args(1)
                .value_parser(NonEmptyStringValueParser::new()),
        )
        .arg(
            Arg::new(FLAG_INSTALLED)
                .short('i')
                .long("installed")
                .num_args(0)
                .help("Search among installed packages only"),
        )
        .arg(
            Arg::new(FLAG_PROVIDES)
                .short('p')
                .long("provides")
                .num_args(0..=1)
                .require_equals(true)
                .default_missing_value("binary")
                .value_parser([
                    "library",
                    "name",
                    "soname",
                    "pkgconfig",
                    "interpreter",
                    "cmake",
                    "python",
                    "binary",
                    "sysbinary",
                    "pkgconfig32",
                ])
                .help("Search for packages by provider"),
        )
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Display)]
#[strum(serialize_all = "lowercase")]
enum MatchKind {
    Name,
    Summary,
}

fn map_aliases(value: &str) -> &str {
    match value {
        "library" => "soname",
        _ => value,
    }
}

fn determine_provider(args: &ArgMatches) -> Result<Provider, Error> {
    let keyword = args.get_one::<String>(ARG_KEYWORD).unwrap();
    let kind = args
        .get_one::<String>(FLAG_PROVIDES)
        .map(|s| map_aliases(s))
        .map(|s| s.parse::<dependency::Kind>().expect("clap should restrict input"));
    Provider::from_name(keyword)
        .map_err(|_| Error::ParseError(keyword.to_owned()))
        .map(|provider| Provider {
            kind: kind.unwrap_or(provider.kind),
            ..provider
        })
}

pub fn handle(args: &ArgMatches, installation: Installation) -> Result<(), Error> {
    let only_installed = args.get_flag(FLAG_INSTALLED);
    let provider = determine_provider(args)?;

    let client = Client::new(environment::NAME, installation)?;
    let flags = if only_installed {
        package::Flags::new().with_installed()
    } else {
        package::Flags::new().with_available()
    };

    let mut output = match provider {
        Provider {
            kind: dependency::Kind::PackageName,
            name,
        } => search_packages(&client, flags, &name),
        Provider {
            kind: _kind,
            name: ref _name,
        } => provides_package(&client, flags, provider),
    }?;

    if output.values().all(|pkgs| pkgs.is_empty()) {
        return Ok(());
    }

    for (match_kind, value) in output.iter_mut() {
        println!("Matched field: {match_kind}");
        value.sort();
        print_columns(value, 1);
    }

    Ok(())
}

fn search_packages(
    client: &Client,
    flags: package::Flags,
    keyword: &str,
) -> Result<BTreeMap<MatchKind, Vec<Output>>, Error> {
    let mut output_kind: BTreeMap<MatchKind, Vec<Output>> = BTreeMap::new();

    for pkg in client.search_packages(keyword, flags) {
        let pkg_name_lowercase = pkg.meta.name.as_str().to_ascii_lowercase();
        let match_kind = if pkg_name_lowercase.contains(&keyword.to_ascii_lowercase()) {
            MatchKind::Name
        } else {
            MatchKind::Summary
        };
        output_kind.entry(match_kind).or_default().push(Output {
            name: pkg.meta.name,
            summary: pkg.meta.summary,
            search_match: Some(keyword.to_owned()),
        });
    }
    Ok(output_kind)
}

fn provides_package(
    client: &Client,
    flags: package::Flags,
    provider: Provider,
) -> Result<BTreeMap<MatchKind, Vec<Output>>, Error> {
    let mut result: BTreeMap<MatchKind, Vec<Output>> = BTreeMap::new();
    let packages = client.lookup_packages_by_provider(&provider, flags);
    result.insert(MatchKind::Name, packages.into_iter().map(Output::from).collect());
    Ok(result)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("client")]
    Client(#[from] client::Error),

    #[error("Invalid dependency type: {0}")]
    ParseError(String),
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Output {
    name: Name,
    summary: String,
    search_match: Option<String>,
}

fn highlight_string(content: &str, expression: &str) -> (String, String, String) {
    if let Some(index) = content.to_ascii_lowercase().find(&expression.to_ascii_lowercase()) {
        let (prefix, body) = content.split_at(index);
        let (matched, suffix) = body.split_at(expression.len());
        (prefix.to_owned(), matched.to_owned(), suffix.to_owned())
    } else {
        (content.to_owned(), String::default(), String::default())
    }
}

impl ColumnDisplay for Output {
    fn get_display_width(&self) -> usize {
        self.name.as_str().chars().count()
    }

    fn display_column(&self, writer: &mut impl std::io::prelude::Write, _col: tui::pretty::Column, width: usize) {
        if let Some(expression) = self.search_match.as_deref() {
            let (name_prefix, name_matched, name_suffix) = highlight_string(self.name.as_str(), expression);
            let (summary_prefix, summary_matched, summary_suffix) = highlight_string(&self.summary, expression);
            let _ = write!(
                writer,
                " {}{}{}{:width$}  {}{}{}",
                name_prefix.bold(),
                name_matched.bold().green(),
                name_suffix.bold(),
                " ".repeat(width),
                summary_prefix.bold(),
                summary_matched.bold().green(),
                summary_suffix.bold(),
            );
        } else {
            let _ = write!(
                writer,
                " {}{:width$}  {}",
                self.name.as_str().bold(),
                " ".repeat(width),
                self.summary
            );
        }
    }
}

impl From<package::Package> for Output {
    fn from(pkg: package::Package) -> Self {
        Output {
            name: pkg.meta.name,
            summary: pkg.meta.summary,
            search_match: None,
        }
    }
}

#[cfg(test)]
mod tests {

    use moss::Registry;
    use moss::package::{self, Name, Package};
    use moss::registry::plugin;
    use std::collections::BTreeSet;
    use std::sync::LazyLock;

    use super::*;

    fn pkg(name: &str, summary: &str, providers: BTreeSet<Provider>) -> Package {
        Package {
            id: package::Id::from(name.to_owned()),
            meta: package::Meta {
                name: Name::from(name.to_owned()),
                summary: summary.to_owned(),
                providers,
                version_identifier: Default::default(),
                source_release: Default::default(),
                build_release: Default::default(),
                architecture: Default::default(),
                description: Default::default(),
                source_id: Default::default(),
                homepage: Default::default(),
                licenses: Default::default(),
                dependencies: Default::default(),
                conflicts: Default::default(),
                uri: Default::default(),
                hash: Default::default(),
                download_size: Default::default(),
            },
            flags: package::Flags::new().with_available(),
        }
    }

    fn test_registry() -> Registry {
        let mut registry = Registry::default();

        let jq = pkg(
            "jq",
            "Command-line JSON processor",
            BTreeSet::from([
                Provider {
                    kind: dependency::Kind::PackageName,
                    name: "jq".to_owned(),
                },
                Provider {
                    kind: dependency::Kind::Binary,
                    name: "jq".to_owned(),
                },
            ]),
        );

        let helix = pkg(
            "helix",
            "A post-modern text editor",
            BTreeSet::from([
                Provider {
                    kind: dependency::Kind::PackageName,
                    name: "helix".to_owned(),
                },
                Provider {
                    kind: dependency::Kind::Binary,
                    name: "hx".to_owned(),
                },
            ]),
        );

        registry.add_plugin(plugin::Plugin::Test(plugin::Test::new(1, vec![jq, helix])));
        registry
    }

    static TEST_CLIENT: LazyLock<Option<Client>> = LazyLock::new(|| {
        let root = tempfile::tempdir().unwrap();
        let installation = Installation::open(root.path(), None).unwrap();
        let registry = test_registry();
        Client::mocked(installation, registry).ok()
    });

    #[test]
    fn test_search() {
        for pkg in ["jq", "hx"] {
            let args = command().try_get_matches_from(["search", pkg]).unwrap();
            let provider = determine_provider(&args).unwrap();
            assert_eq!(provider.kind, dependency::Kind::PackageName);
        }
    }

    #[test]
    fn test_search_with_kind() {
        for pkg in ["jq", "hx"] {
            let args = command()
                .try_get_matches_from(["search", "--provides=binary", pkg])
                .unwrap();
            let provider = determine_provider(&args).unwrap();
            assert_eq!(provider.kind, dependency::Kind::Binary);
        }
    }

    #[test]
    fn test_search_with_provider_syntax() {
        for pkg in ["jq", "hx"] {
            let args = command()
                .try_get_matches_from(["search", format!("binary({pkg})").as_str()])
                .unwrap();
            let provider = determine_provider(&args).unwrap();
            assert_eq!(provider.kind, dependency::Kind::Binary);
        }
    }
}
