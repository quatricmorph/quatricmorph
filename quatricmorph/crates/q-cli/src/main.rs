use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "q")]
#[command(about = "Quatricmorph CLI tool")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the daemon
    Daemon,
    /// List available models
    Models,
    /// Run an inference
    Run {
        /// Model name
        #[arg(short, long)]
        model: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Daemon) => println!("Starting daemon..."),
        Some(Commands::Models) => println!("Listing models..."),
        Some(Commands::Run { model }) => println!("Running with model: {}", model),
        None => println!("Use --help for usage information"),
    }
}
