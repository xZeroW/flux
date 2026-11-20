pub mod downloads;
pub mod settings;
pub mod splash;

#[derive(Default)]
pub enum Screen {
    #[default]
    Splash,
    Downloads,
    Settings,
}
