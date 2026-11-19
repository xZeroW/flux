use anyhow::Result;
use dm_core::App;

#[tokio::main]
async fn main() -> Result<()> {
    let mut app = App::default();
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal).await;
    ratatui::restore();
    result
}
