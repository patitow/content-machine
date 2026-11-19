use std::path::PathBuf;

use cm_cli::Config;

#[test]
fn absent_file_gives_defaults() {
    let d = tempfile::tempdir().unwrap();
    let c = Config::load_from(&d.path().join("nao-existe.toml")).unwrap();
    assert_eq!(c.obs_dir, PathBuf::from(r"E:\.VIDEOS GRAVADOS\OBS"));
    assert_eq!(
        c.cache_dir,
        PathBuf::from(r"E:\.VIDEOS GRAVADOS\content-machine-cache")
    );
    assert_eq!(c.db_path, c.cache_dir.join("cortes.db"));
}

#[test]
fn partial_file_overrides_only_given_keys_and_derives_db_path() {
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("config.toml");
    std::fs::write(&f, "obs_dir = 'D:/videos'\ncache_dir = 'D:/cache'\n").unwrap();
    let c = Config::load_from(&f).unwrap();
    assert_eq!(c.obs_dir, PathBuf::from("D:/videos"));
    assert_eq!(c.db_path, PathBuf::from("D:/cache").join("cortes.db"));
}

#[test]
fn malformed_file_is_an_error() {
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("config.toml");
    std::fs::write(&f, "obs_dir = [isto não é toml").unwrap();
    assert!(Config::load_from(&f).is_err());
}
