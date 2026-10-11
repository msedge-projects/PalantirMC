#[derive(Clone, Copy)]
pub enum Key {
    Home,
    Library,
    Discover,
    Settings,
    About,
}

pub fn message(key: Key, _language: &str) -> &'static str {
    // Only an independently authored English catalogue is available; future tags fall back.
    match key {
        Key::Home => "Home",
        Key::Library => "Collections",
        Key::Discover => "Explore",
        Key::Settings => "Preferences",
        Key::About => "About",
    }
}
