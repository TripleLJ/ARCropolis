use smash::hash40;
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

lazy_static::lazy_static! {
    pub static ref ARC_FILES: ArcFiles = ArcFiles::new();
}

pub struct ArcFiles(pub HashMap<u64, PathBuf>);

const ARC_DIR: &str = "rom:/arc";
const UMM_DIR: &str = "sd:/ultimate/mods";

impl ArcFiles {
    fn new() -> Self {
        let mut instance = Self(HashMap::new());

        let _ = instance.visit_dir(Path::new(ARC_DIR), ARC_DIR.len());
        let _ = instance.visit_umm_dirs(Path::new(UMM_DIR));

        instance
    }

    /// Visit Ultimate Mod Manager directories for backwards compatibility
    fn visit_umm_dirs(&mut self, dir: &Path) -> io::Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let filename = entry.path();
                let real_path = format!("{}/{}", dir.display(), filename.display());
                let path = Path::new(&real_path);
                if path.is_dir() {
                    self.visit_dir(&path, real_path.len())?;
                }
            }
        }

        Ok(())
    }

    fn visit_dir(&mut self, dir: &Path, arc_dir_len: usize) -> io::Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let filename = entry.path();
                let real_path = format!("{}/{}", dir.display(), filename.display());
                let path = Path::new(&real_path);
                if path.is_dir() {
                    self.visit_dir(&path, arc_dir_len)?;
                } else {
                    self.visit_file(path, arc_dir_len);
                }
            }
        }

        Ok(())
    }

    fn visit_file(&mut self, path: &Path, arc_dir_len: usize) {
        let game_path = path.display().to_string()[arc_dir_len + 1..].replace(";", ":");
        let hash = hash40(&game_path);
        self.0.insert(hash, path.to_owned());
    }

    pub fn get_from_hash(&self, hash: u64) -> Option<&PathBuf> {
        self.0.get(&hash)
    }
}
