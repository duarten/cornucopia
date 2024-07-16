use miette::Diagnostic;
use std::{fs, path::PathBuf};
use thiserror::Error as ThisError;

use clap::{Parser, Subcommand};

use crate::{
    config::Config, conn, container, error::Error, generate_live, generate_managed, CodegenSettings,
};

/// Command line interface to interact with Cornucopia SQL.
#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
    /// Use `podman` instead of `docker`
    #[clap(short, long)]
    podman: bool,
    /// Folder containing the queries
    #[clap(short, long, default_value = "queries/")]
    queries_path: PathBuf,
    /// Destination folder for generated modules
    #[clap(short, long, default_value = "src/cornucopia.rs")]
    destination: PathBuf,
    #[clap(subcommand)]
    action: Action,
    /// Generate synchronous rust code
    #[clap(long)]
    sync: bool,
    /// Generate asynchronous rust code
    #[clap(long)]
    r#async: bool,
    /// Derive serde's `Serialize` trait for generated types.
    #[clap(long)]
    serialize: bool,
    /// The location of the configuration file.
    #[clap(short, long, default_value = default_config_path())]
    config: PathBuf,
}

const fn default_config_path() -> &'static str {
    "cornucopia.toml"
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Generate your modules against your own db
    Live {
        /// Postgres url to the database
        url: String,
    },
    /// Generate your modules against schema files
    Schema {
        /// SQL files containing the database schema
        schema_files: Vec<PathBuf>,
    },
}

/// Enumeration of the errors reported by the CLI.
#[derive(ThisError, Debug, Diagnostic)]
pub enum CliError {
    /// An error occurred while loading the configuration file.
    #[error("Could not load config `{path}`: ({err})")]
    MissingConfig { path: String, err: std::io::Error },
    /// An error occurred while parsing the configuration file.
    #[error("Could not parse config `{path}`: ({err})")]
    ConfigContents {
        path: String,
        err: Box<dyn std::error::Error + Send + Sync>,
    },
    /// An error occurred while running the CLI.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Internal(#[from] Error),
}

// Main entrypoint of the CLI. Parses the args and calls the appropriate routines.
pub fn run() -> Result<(), CliError> {
    let Args {
        podman,
        queries_path,
        destination,
        action,
        sync,
        r#async,
        serialize,
        config,
    } = Args::parse();

    let config = match fs::read_to_string(config.as_path()) {
        Ok(contents) => match toml::from_str(&contents) {
            Ok(config) => config,
            Err(err) => {
                return Err(CliError::ConfigContents {
                    path: config.to_string_lossy().into_owned(),
                    err: err.into(),
                });
            }
        },
        Err(err) => {
            if config.as_path().as_os_str() != default_config_path() {
                return Err(CliError::MissingConfig {
                    path: config.to_string_lossy().into_owned(),
                    err,
                });
            } else {
                Config::default()
            }
        }
    };
    let settings = CodegenSettings {
        gen_async: r#async || !sync,
        gen_sync: sync,
        derive_ser: serialize,
        config,
    };

    match action {
        Action::Live { url } => {
            let mut client = conn::from_url(&url).map_err(|e| CliError::Internal(e.into()))?;
            generate_live(&mut client, &queries_path, Some(&destination), settings)?;
        }
        Action::Schema { schema_files } => {
            // Run the generate command. If the command is unsuccessful, cleanup Cornucopia's container
            if let Err(e) = generate_managed(
                queries_path,
                &schema_files,
                Some(destination),
                podman,
                settings,
            ) {
                container::cleanup(podman).ok();
                return Err(CliError::Internal(e));
            }
        }
    };
    Ok(())
}
