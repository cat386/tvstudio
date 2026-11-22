use super::*;

#[test]
fn clone_destination_names_repository_folder() {
    let parent = Path::new("workspace");
    for url in [
        "https://example.test/team/project.git",
        "ssh://git@example.test:2222/team/project",
        "git@example.test:team/project.git",
        "D:\\repos\\project.git",
        "/home/user/project/.git",
    ] {
        assert_eq!(clone_destination(parent, url).unwrap(), parent.join("project"), "{url}");
    }
    assert!(clone_destination(parent, "https://user:secret@example.test/project.git").is_err());
    assert!(clone_destination(parent, "https://example.test/..").is_err());
}
