use anyhow::Result;
use dm_core::App;

#[tokio::main]
async fn main() -> Result<()> {
    let mut app = App::new()?;
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal).await;
    ratatui::restore();
    result
}
