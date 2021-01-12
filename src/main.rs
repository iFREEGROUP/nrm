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
            println!("Updating...please wait.");

            nrm::update_lockfile(
                &path.map(PathBuf::from).unwrap_or_else(|| {
                    let mut cwd =
                        env::current_dir().expect("failed to retrieve current working directory");
                    cwd.push("package-lock.json");
                    cwd
                }),
                registry.trim_end_matches("/"),
            )
            .await
        }
        Command::Check(_) => unimplemented!(),
    }
}
