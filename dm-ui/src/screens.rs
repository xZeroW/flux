pub mod downloads;
pub mod splash;

#[derive(Default)]
pub enum Screen {
    #[default]
    Splash,
    Downloads,
}
