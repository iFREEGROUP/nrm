#[tokio::main]
async fn main() -> anyhow::Result<()> {
    nrm::update_lockfile("./package-lock.json", "https://registry.npmjs.org").await?;

    Ok(())
}
