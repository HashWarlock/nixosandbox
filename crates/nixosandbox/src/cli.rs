use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "nixosandbox", about = "Reproducible, isolated sandbox environments")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a new sandbox session
    Create {
        /// Use a built-in profile
        #[arg(long)]
        profile: Option<String>,

        /// Use a custom spec file
        #[arg(long)]
        spec: Option<String>,

        /// Host directory to mount as /workspace
        #[arg(long)]
        workspace: Option<String>,

        /// Human-readable session name
        #[arg(long)]
        name: Option<String>,

        /// Agent runtime identifier (e.g. 'claude:opus-4-6')
        #[arg(long)]
        agent: Option<String>,

        /// Purpose of this sandbox session
        #[arg(long)]
        description: Option<String>,

        /// Output session info as JSON
        #[arg(long)]
        json: bool,
    },

    /// Execute a command inside a sandbox
    Exec {
        /// Session ID
        session_id: String,

        /// Stream NDJSON events
        #[arg(long)]
        json: bool,

        /// Kill after timeout (seconds)
        #[arg(long)]
        timeout: Option<u64>,

        /// Additional environment variable (KEY=VALUE)
        #[arg(long = "env", value_name = "KEY=VALUE")]
        extra_env: Vec<String>,

        /// Command to execute (after --)
        #[arg(last = true)]
        command: Vec<String>,
    },

    /// Enter a sandbox interactively
    Enter {
        /// Session ID
        session_id: String,
    },

    /// List active sandbox sessions
    List {
        /// Output as JSON array
        #[arg(long)]
        json: bool,
    },

    /// Destroy a sandbox session
    Destroy {
        /// Session ID
        session_id: String,
    },

    /// Show detailed session status (battlecard)
    Status {
        /// Session ID
        session_id: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Build a rootfs without creating a session
    Build {
        /// Use a built-in profile
        #[arg(long)]
        profile: Option<String>,

        /// Use a custom spec file
        #[arg(long)]
        spec: Option<String>,

        /// Output rootfs path as JSON
        #[arg(long)]
        json: bool,
    },

}
