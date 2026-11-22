use super::*;

#[test]
fn replaces_existing_and_preserves_target_on_failure() {
    let dir = std::env::temp_dir().join(format!("tvs_atomic_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("form.tvf");
    write_atomic(&path, "старое").unwrap();
    write_atomic(&path, "новое").unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "новое");
    let permissions = fs::metadata(&path).unwrap().permissions();
    let mut readonly = permissions.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let result = write_atomic(&path, "не должно записаться");
    fs::set_permissions(&path, permissions).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "новое");
    let blocked = dir.join("directory");
    fs::create_dir_all(&blocked).unwrap();
    assert!(write_atomic(&blocked, "не заменит каталог").is_err());
    assert!(blocked.is_dir());
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 2, "временный файл удалён после ошибки");
    fs::remove_dir_all(dir).unwrap();
}
