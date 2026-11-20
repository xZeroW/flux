pub mod downloads;
pub mod provider;
pub mod settings;
pub mod splash;

#[derive(Default)]
pub enum Screen {
    #[default]
    Splash,
    Downloads,
    Settings,
    Provider,
}
