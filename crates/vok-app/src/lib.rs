pub mod app;
pub mod launcher;
pub mod overlay;
pub mod stage;
pub mod ui;

pub use libadwaita as adw;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adw_schemes() {
        assert_eq!(adw::ColorScheme::Default, adw::ColorScheme::Default);
        assert_eq!(adw::ColorScheme::ForceDark, adw::ColorScheme::ForceDark);
        assert_eq!(adw::ColorScheme::ForceLight, adw::ColorScheme::ForceLight);
    }
}


