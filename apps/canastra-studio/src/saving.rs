//! Saving a data file without ever leaving a broken one behind.

use std::fs;
use std::path::Path;

use canastra_data::format;

/// Writes next to the target first, keeps the previous file as `.bak`, then swaps the new one in,
/// so a failure at any point never leaves a half-written data file.
pub(crate) fn write_safely(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temp = path.with_extension("cana.tmp");
    fs::write(&temp, bytes)?;
    // Reading the written file back proves the bytes on disk decode before anything is replaced.
    if format::decode(&fs::read(&temp)?).is_err() {
        return Err(std::io::Error::other("written data does not read back"));
    }
    if path.exists() {
        fs::copy(path, path.with_extension("cana.bak"))?;
    }
    fs::rename(&temp, path)
}

#[cfg(test)]
mod tests {
    use canastra_data::GameData;
    use canastra_data::id::ItemId;
    use canastra_data::item::Item;

    use super::*;

    #[test]
    fn saving_keeps_the_previous_file() {
        let dir = std::env::temp_dir().join(format!("canastra-studio-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("data.cana");
        let first = GameData::default();
        let mut second = GameData::default();
        second.items.insert(ItemId(1), Item::new(ItemId(1)));

        write_safely(&path, &format::encode(&first)).unwrap();
        write_safely(&path, &format::encode(&second)).unwrap();

        assert_eq!(format::decode(&fs::read(&path).unwrap()).unwrap(), second);
        assert_eq!(format::decode(&fs::read(path.with_extension("cana.bak")).unwrap()).unwrap(), first);
        assert!(!path.with_extension("cana.tmp").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
