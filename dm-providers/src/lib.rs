pub mod direct_download;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCommand {
    pub name: &'static str,
    pub description: &'static str,
}

pub fn commands() -> &'static [ProviderCommand] {
    direct_download::commands()
}
