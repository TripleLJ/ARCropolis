pub mod changelog;
pub mod mods;
pub mod workspaces;
pub mod settings;

pub mod paths {
    use camino::Utf8PathBuf;

    pub fn mods() -> Utf8PathBuf {
        Utf8PathBuf::from("sd:/ultimate/mods")
    }
}
