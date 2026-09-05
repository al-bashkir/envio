use clap::Parser;
use clap_complete::Shell;

#[derive(Parser, Debug)]

/// envio is a modern and secure CLI tool that simplifies the management of
/// environment variables.
pub struct ClapApp {
    #[command(subcommand)]
    pub command: Command,
}

/// List of all possible `subcommands` for the application
/// When a subcommand is passed to the application, clap returns the corresponding enum variant
/// The enum variant then calls the run method which is implemented in the command.rs file.
///
/// When adding a new subcommand, add it to this enum and then write a match arm in the run method
/// which is located in the command.rs file
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    #[command(
        name = "create",
        about = "Create a new profile",
        override_usage = "envio create <PROFILE_NAME> [OPTIONS]"
    )]
    Create {
        #[arg(required = true, help = "Name of the profile to create")]
        profile_name: String,
        #[arg(
            required = false,
            long = "file-to-import-envs-from",
            short = 'f',
            help = "Read the environment variables from a .env style file"
        )]
        envs_file: Option<String>,
        #[arg(
            required = false,
            long = "envs",
            short = 'e',
            value_delimiter = ' ',
            num_args = 1..,
            help = "Environment variables to store, as space separated KEY=VALUE pairs",
        )]
        envs: Option<Vec<String>>,
        #[arg(
            required = false,
            long = "gpg-key-fingerprint",
            short = 'g',
            help = "Encrypt with this GPG key instead of a passphrase"
        )]
        gpg: Option<String>,
        #[arg(
            required = false,
            long = "add-comments",
            short = 'c',
            help = "Prompt for a comment for each environment variable"
        )]
        add_comments: bool,
        #[arg(
            required = false,
            long = "add-expiration-date",
            short = 'x',
            help = "Prompt for an expiration date for each environment variable"
        )]
        add_expiration_date: bool,
    },
    #[command(
        name = "add",
        about = "Add envionment variables to a profile",
        override_usage = "envio add <PROFILE_NAME> [OPTIONS]"
    )]
    Add {
        #[arg(required = true, help = "Name of the profile to add to")]
        profile_name: String,
        #[arg(
            required = true,
            long = "envs",
            short = 'e',
            value_delimiter = ' ',
            num_args = 1..,
            help = "Environment variables to add, as space separated KEY=VALUE pairs",
        )]
        envs: Vec<String>,
        #[arg(
            required = false,
            long = "add-comments",
            short = 'c',
            help = "Prompt for a comment for each environment variable"
        )]
        add_comments: bool,
        #[arg(
            required = false,
            long = "add-expiration-date",
            short = 'x',
            help = "Prompt for an expiration date for each environment variable"
        )]
        add_expiration_date: bool,
    },
    #[command(
        name = "load",
        about = "Load all environment variables in a profile for use in your terminal sessions"
    )]
    Load {
        #[arg(required = true, help = "Name of the profile to load")]
        profile_name: String,
    },
    #[command(name = "unload", about = "Unload a profile")]
    Unload {
        #[arg(required = true, help = "Name of the profile to unload")]
        profile_name: String,
    },
    #[command(
        name = "launch",
        about = "Run a command with the environment variables from a profile",
        override_usage = "envio launch <PROFILE_NAME> <--command STRING_COMMAND | -- COMMAND>"
    )]
    Launch {
        #[arg(required = true, help = "Name of the profile to run the command with")]
        profile_name: String,
        #[command(flatten)]
        command: LaunchCommandArg,
    },
    #[command(
        name = "remove",
        about = "Remove a environment variable from a profile",
        override_usage = "envio remove <PROFILE_NAME> [OPTIONS]"
    )]
    Remove {
        #[arg(
            required = true,
            help = "Name of the profile to remove from, or to delete when no variables are given"
        )]
        profile_name: String,
        #[arg(required = false, long = "envs-to-remove", short = 'e', value_delimiter = ' ', num_args = 1..,
            help = "Names of the environment variables to remove; omit to delete the whole profile")]
        envs: Option<Vec<String>>,
    },
    #[command(
        name = "list",
        about = "List all the environment variables in a profile or all the profiles currenty stored"
    )]
    List {
        #[arg(
            required = false,
            long = "profiles",
            short = 'p',
            help = "List the stored profiles instead of environment variables"
        )]
        profiles: bool,
        #[arg(
            required = false,
            long = "profile-name",
            short = 'n',
            help = "Name of the profile whose environment variables to list"
        )]
        profile_name: Option<String>,
        #[arg(
            required = false,
            long = "no-pretty-print",
            short = 'v',
            help = "Print plain lines instead of a formatted table"
        )]
        no_pretty_print: bool,
        #[arg(
            required = false,
            long = "display-comments",
            short = 'c',
            help = "Show the comment attached to each environment variable"
        )]
        display_comments: bool,
        #[arg(
            required = false,
            long = "display-expiration-date",
            short = 'x',
            help = "Show the expiration date of each environment variable"
        )]
        display_expiration_date: bool,
    },
    #[command(
        name = "update",
        about = "Update environment variables in a profile",
        override_usage = "envio update <PROFILE_NAME> [OPTIONS]"
    )]
    Update {
        #[arg(required = true, help = "Name of the profile to update")]
        profile_name: String,
        #[arg(
            required = true,
            long = "envs",
            short = 'e',
            value_delimiter = ' ',
            num_args = 1..,
            help = "Names of the environment variables to update, space separated",
        )]
        envs: Vec<String>,
        #[arg(
            required = false,
            long = "update-values",
            short = 'v',
            help = "Prompt for a new value for each environment variable"
        )]
        update_values: bool,
        #[arg(
            required = false,
            long = "update-comments",
            short = 'c',
            help = "Prompt for a new comment for each environment variable"
        )]
        update_comments: bool,
        #[arg(
            required = false,
            long = "update-expiration-date",
            short = 'x',
            help = "Prompt for a new expiration date for each environment variable"
        )]
        update_expiration_date: bool,
    },
    #[command(
        name = "export",
        about = "Export a profile to a file if no file is specified it will be exported to a file named .env",
        override_usage = "envio export <PROFILE_NAME> [OPTIONS]"
    )]
    Export {
        #[arg(required = true, help = "Name of the profile to export")]
        profile_name: String,
        #[arg(
            required = false,
            long = "file-to-export-to",
            short = 'f',
            help = "Path to write to (defaults to .env)"
        )]
        file: Option<String>,
        #[arg(
            required = false,
            long = "envs",
            short = 'e',
            value_delimiter = ' ',
            num_args = 1..,
            help = "Names of the environment variables to export; omit to export all",
        )]
        envs: Option<Vec<String>>,
    },
    #[command(
        name = "import",
        about = "Download a profile over the internet and import it into the system or import a locally stored profile into your current envio installation",
        override_usage = "envio import <PROFILE_NAME> [OPTIONS]"
    )]
    Import {
        #[arg(required = true, help = "Name to store the imported profile under")]
        profile_name: String,
        #[arg(
            required = false,
            long = "file-to-import-from",
            short = 'f',
            help = "Path to a locally stored profile file"
        )]
        file: Option<String>,
        #[arg(
            required = false,
            long = "url",
            short = 'u',
            help = "URL to download the profile from"
        )]
        url: Option<String>,
    },
    #[command(
        name = "sync",
        about = "Push and pull encrypted profiles to a remote store (S3, Google Drive, or a directory)"
    )]
    Sync {
        #[command(subcommand)]
        action: SyncAction,
    },
    #[command(name = "version", about = "Print the version")]
    Version {
        #[arg(
            required = false,
            long = "verbose",
            short = 'v',
            help = "Also print the build target, timestamp and commit"
        )]
        verbose: bool,
        #[arg(
            required = false,
            long = "check",
            help = "Check for a newer release (requires network)"
        )]
        check: bool,
    },
    #[command(
        name = "completion",
        about = "Generate shell completion scripts",
        override_usage = "envio completion <SHELL>"
    )]
    Completion {
        #[arg(
            required = true,
            value_enum,
            help = "Shell to generate the completion script for"
        )]
        shell: Shell,
    },
}

#[derive(clap::Args, Debug)]
#[group(required = true, multiple = false)]
pub struct LaunchCommandArg {
    #[arg(
        long = "command",
        short = 'c',
        name = "STRING_COMMAND",
        help = "Command to run, as a single string"
    )]
    argument: Option<String>,
    #[arg(
        last = true,
        name = "COMMAND",
        help = "Command to run, given after a -- separator"
    )]
    positional: Vec<String>,
}

impl LaunchCommandArg {
    pub fn value(&self) -> Vec<&str> {
        if let Some(command) = self.argument.as_ref() {
            return command.split_whitespace().collect();
        }
        self.positional.iter().map(|s| s.as_str()).collect()
    }
}

#[derive(clap::Subcommand, Debug)]
pub enum SyncAction {
    #[command(about = "Manage sync remotes")]
    Remote {
        #[command(subcommand)]
        action: RemoteAction,
    },
    #[command(
        about = "Upload profiles to the remote (all local profiles if none given)",
        override_usage = "envio sync push [PROFILE]... [OPTIONS]"
    )]
    Push {
        #[arg(help = "Profiles to upload; omit to upload all local profiles")]
        profiles: Vec<String>,
        #[arg(long, short = 'r', help = "Remote name from sync.toml")]
        remote: Option<String>,
        #[arg(long, short = 'f', help = "Overwrite even if the remote changed")]
        force: bool,
    },
    #[command(
        about = "Download profiles from the remote (all remote profiles if none given)",
        override_usage = "envio sync pull [PROFILE]... [OPTIONS]"
    )]
    Pull {
        #[arg(help = "Profiles to download; omit to download all remote profiles")]
        profiles: Vec<String>,
        #[arg(long, short = 'r', help = "Remote name from sync.toml")]
        remote: Option<String>,
        #[arg(long, short = 'f', help = "Overwrite even if the local copy changed")]
        force: bool,
    },
    #[command(about = "Show how each profile compares with the remote")]
    Status {
        #[arg(long, short = 'r', help = "Remote name from sync.toml")]
        remote: Option<String>,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum RemoteAction {
    #[command(about = "Add a remote interactively")]
    Add {
        #[arg(required = true, help = "Name for the new remote")]
        name: String,
    },
    #[command(about = "List configured remotes")]
    List,
    #[command(about = "Remove a remote")]
    Remove {
        #[arg(required = true, help = "Name of the remote to remove")]
        name: String,
    },
}
