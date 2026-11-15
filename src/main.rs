use anyhow::Result;
use tui_file_downloader::core::app::App;

#[tokio::main]
async fn main() -> Result<()> {
    let mut app = App::new().await?;

    app.run().await?;

    Ok(())
}
