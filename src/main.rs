#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    nrm::update_lockfile("./package-lock.json", "https://registry.npmjs.org").await?;

    Ok(())
}
