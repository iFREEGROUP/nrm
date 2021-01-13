use std::{env, path::PathBuf};
use structopt::StructOpt;

#[derive(StructOpt)]
enum Command {
    /// Update registry and write to `package-lock.json` file
    Write(Opt),
    /// Check if a `package-lock.json` file use specific registry
    Check(Opt),
}

#[derive(StructOpt)]
struct Opt {
    /// Registry URL
    #[structopt(long)]
    registry: String,

    /// Specify path of `package-lock.json`. Default is current directory.
    #[structopt(long)]
    path: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    match Command::from_args() {
        Command::Write(Opt { registry, path }) => {
            println!("Updating, please wait. This will take a few minutes, depending on your network and amount of packages.");

            nrm::update_lockfile(
                &path
                    .map(PathBuf::from)
                    .unwrap_or_else(default_lock_file_path),
                registry.trim_end_matches("/"),
            )
            .await
        }
        Command::Check(Opt { registry, path }) => nrm::check_lockfile(
            &path
                .map(PathBuf::from)
                .unwrap_or_else(default_lock_file_path),
            registry.trim_end_matches("/"),
        )
        .await
        .map(|_| println!("Yay! This lockfile has already used specific registry.")),
    }
}

fn default_lock_file_path() -> PathBuf {
    let mut cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    cwd.push("package-lock.json");
    cwd
}
