use super::*;

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lux-ui-file-tree-test-{:?}-{}",
        std::thread::current().id(),
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn names(rows: &[Row]) -> Vec<&str> {
    rows.iter().map(|r| r.name.as_str()).collect()
}

#[test]
fn open_loads_and_expands_the_root_directory() {
    let dir = temp_dir();
    fs::write(dir.join("b.rs"), "").unwrap();
    fs::create_dir(dir.join("a_dir")).unwrap();

    let mut tree = FileTree::open(&dir).unwrap();
    assert_eq!(tree.node_kind(tree.root()), Some(NodeKind::Directory));
    assert_eq!(tree.is_expanded(tree.root()), Some(true));
    assert_eq!(
        names(tree.rows()),
        vec![display_name(&dir).as_str(), "a_dir", "b.rs"]
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn open_on_a_file_creates_an_unexpandable_root() {
    let dir = temp_dir();
    let file = dir.join("solo.txt");
    fs::write(&file, "hi").unwrap();

    let mut tree = FileTree::open(&file).unwrap();
    assert_eq!(tree.node_kind(tree.root()), Some(NodeKind::File));
    assert_eq!(tree.rows().len(), 1);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn children_sort_directories_first_then_case_insensitively() {
    let dir = temp_dir();
    fs::write(dir.join("Zebra.txt"), "").unwrap();
    fs::write(dir.join("apple.txt"), "").unwrap();
    fs::create_dir(dir.join("Nested")).unwrap();

    let mut tree = FileTree::open(&dir).unwrap();
    assert_eq!(
        names(tree.rows()),
        vec![
            display_name(&dir).as_str(),
            "Nested",
            "apple.txt",
            "Zebra.txt"
        ]
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn expand_caches_children_across_collapse_and_re_expand() {
    let dir = temp_dir();
    fs::create_dir(dir.join("sub")).unwrap();

    let mut tree = FileTree::open(&dir).unwrap();
    let sub_id = tree.rows()[1].id;
    tree.expand(sub_id).unwrap();
    let first_ids: Vec<NodeId> = tree.rows().iter().map(|r| r.id).collect();

    tree.collapse(sub_id);
    tree.expand(sub_id).unwrap();
    let second_ids: Vec<NodeId> = tree.rows().iter().map(|r| r.id).collect();

    assert_eq!(first_ids, second_ids);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn collapse_hides_descendant_rows_without_forgetting_them() {
    let dir = temp_dir();
    fs::create_dir(dir.join("sub")).unwrap();
    fs::write(dir.join("sub/inner.txt"), "").unwrap();

    let mut tree = FileTree::open(&dir).unwrap();
    let sub_id = tree.rows()[1].id;
    tree.expand(sub_id).unwrap();
    assert_eq!(tree.rows().len(), 3);

    tree.collapse(sub_id);
    assert_eq!(tree.rows().len(), 2);

    tree.expand(sub_id).unwrap();
    assert_eq!(tree.rows().len(), 3);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn visible_rows_slices_by_viewport() {
    let dir = temp_dir();
    for name in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        fs::write(dir.join(name), "").unwrap();
    }

    let mut tree = FileTree::open(&dir).unwrap();
    let window = tree.visible_rows(1, 2);
    assert_eq!(names(window), vec!["a.txt", "b.txt"]);

    let clamped = tree.visible_rows(3, 100);
    assert_eq!(names(clamped), vec!["c.txt", "d.txt"]);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn create_file_writes_to_disk_and_appears_in_rows() {
    let dir = temp_dir();
    let mut tree = FileTree::open(&dir).unwrap();
    let root = tree.root();

    let id = tree.create(root, "new.rs", NodeKind::File).unwrap();
    assert!(dir.join("new.rs").is_file());
    assert_eq!(tree.node_path(id), Some(dir.join("new.rs").as_path()));
    assert_eq!(
        names(tree.rows()),
        vec![display_name(&dir).as_str(), "new.rs"]
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn create_file_refuses_to_overwrite_an_existing_file() {
    let dir = temp_dir();
    let mut tree = FileTree::open(&dir).unwrap();
    let root = tree.root();
    tree.create(root, "existing.rs", NodeKind::File).unwrap();
    fs::write(dir.join("existing.rs"), "original content").unwrap();

    let err = tree
        .create(root, "existing.rs", NodeKind::File)
        .unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(
        fs::read_to_string(dir.join("existing.rs")).unwrap(),
        "original content"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn create_directory_writes_to_disk_and_appears_in_rows() {
    let dir = temp_dir();
    let mut tree = FileTree::open(&dir).unwrap();
    let root = tree.root();

    tree.create(root, "new_dir", NodeKind::Directory).unwrap();
    assert!(dir.join("new_dir").is_dir());
    assert_eq!(
        names(tree.rows()),
        vec![display_name(&dir).as_str(), "new_dir"]
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn create_under_a_file_parent_errors() {
    let dir = temp_dir();
    let mut tree = FileTree::open(&dir).unwrap();
    let root = tree.root();
    let file_id = tree.create(root, "leaf.txt", NodeKind::File).unwrap();

    let err = tree
        .create(file_id, "child.txt", NodeKind::File)
        .unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn rename_moves_the_file_on_disk_and_updates_the_path() {
    let dir = temp_dir();
    let mut tree = FileTree::open(&dir).unwrap();
    let root = tree.root();
    let id = tree.create(root, "old.txt", NodeKind::File).unwrap();

    tree.rename(id, "new.txt").unwrap();
    assert!(!dir.join("old.txt").exists());
    assert!(dir.join("new.txt").is_file());
    assert_eq!(tree.node_path(id), Some(dir.join("new.txt").as_path()));

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn rename_refuses_to_overwrite_an_existing_file() {
    let dir = temp_dir();
    let mut tree = FileTree::open(&dir).unwrap();
    let root = tree.root();
    let id = tree.create(root, "source.txt", NodeKind::File).unwrap();
    fs::write(dir.join("target.txt"), "must survive").unwrap();

    let err = tree.rename(id, "target.txt").unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    assert!(dir.join("source.txt").is_file());
    assert_eq!(
        fs::read_to_string(dir.join("target.txt")).unwrap(),
        "must survive"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn rename_updates_descendant_paths_for_a_directory() {
    let dir = temp_dir();
    let mut tree = FileTree::open(&dir).unwrap();
    let root = tree.root();
    let dir_id = tree.create(root, "old_dir", NodeKind::Directory).unwrap();
    tree.expand(dir_id).unwrap();
    let inner_id = tree.create(dir_id, "inner.txt", NodeKind::File).unwrap();

    tree.rename(dir_id, "new_dir").unwrap();
    assert_eq!(
        tree.node_path(inner_id),
        Some(dir.join("new_dir/inner.txt").as_path())
    );
    assert!(dir.join("new_dir/inner.txt").is_file());

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn rename_the_root_errors() {
    let dir = temp_dir();
    let mut tree = FileTree::open(&dir).unwrap();
    let root = tree.root();
    assert_eq!(
        tree.rename(root, "renamed").unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn move_node_moves_the_file_on_disk_to_the_new_parent() {
    let dir = temp_dir();
    let mut tree = FileTree::open(&dir).unwrap();
    let root = tree.root();
    let file_id = tree.create(root, "loose.txt", NodeKind::File).unwrap();
    let dest_id = tree.create(root, "dest", NodeKind::Directory).unwrap();

    tree.move_node(file_id, dest_id).unwrap();
    assert!(!dir.join("loose.txt").exists());
    assert!(dir.join("dest/loose.txt").is_file());
    assert_eq!(
        tree.node_path(file_id),
        Some(dir.join("dest/loose.txt").as_path())
    );

    tree.expand(dest_id).unwrap();
    assert_eq!(
        names(tree.rows()),
        vec![display_name(&dir).as_str(), "dest", "loose.txt"]
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn move_node_refuses_to_overwrite_an_existing_file_at_the_destination() {
    let dir = temp_dir();
    let mut tree = FileTree::open(&dir).unwrap();
    let root = tree.root();
    let file_id = tree.create(root, "loose.txt", NodeKind::File).unwrap();
    let dest_id = tree.create(root, "dest", NodeKind::Directory).unwrap();
    fs::write(dir.join("dest/loose.txt"), "must survive").unwrap();

    let err = tree.move_node(file_id, dest_id).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    assert!(dir.join("loose.txt").is_file());
    assert_eq!(
        fs::read_to_string(dir.join("dest/loose.txt")).unwrap(),
        "must survive"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn move_node_into_its_own_descendant_errors() {
    let dir = temp_dir();
    let mut tree = FileTree::open(&dir).unwrap();
    let root = tree.root();
    let parent_id = tree.create(root, "parent", NodeKind::Directory).unwrap();
    tree.expand(parent_id).unwrap();
    let child_id = tree
        .create(parent_id, "child", NodeKind::Directory)
        .unwrap();

    let err = tree.move_node(parent_id, child_id).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn move_root_errors() {
    let dir = temp_dir();
    let mut tree = FileTree::open(&dir).unwrap();
    let root = tree.root();
    let dest_id = tree.create(root, "dest", NodeKind::Directory).unwrap();
    assert_eq!(
        tree.move_node(root, dest_id).unwrap_err().kind(),
        io::ErrorKind::InvalidInput
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn delete_to_trash_removes_the_file_from_disk_and_from_the_tree() {
    let dir = temp_dir();
    let mut tree = FileTree::open(&dir).unwrap();
    let root = tree.root();
    let id = tree.create(root, "throwaway.txt", NodeKind::File).unwrap();

    tree.delete_to_trash(id).unwrap();
    assert!(!dir.join("throwaway.txt").exists());
    assert_eq!(names(tree.rows()), vec![display_name(&dir)]);
    assert_eq!(tree.node_path(id), None);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn delete_unknown_node_errors() {
    let dir = temp_dir();
    let mut tree = FileTree::open(&dir).unwrap();
    let root = tree.root();
    let id = tree.create(root, "gone.txt", NodeKind::File).unwrap();
    tree.delete_to_trash(id).unwrap();

    assert_eq!(
        tree.delete_to_trash(id).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn reveal_expands_ancestors_and_returns_the_target_id() {
    let dir = temp_dir();
    fs::create_dir_all(dir.join("a/b")).unwrap();
    fs::write(dir.join("a/b/target.txt"), "").unwrap();

    let mut tree = FileTree::open(&dir).unwrap();
    let target = dir.join("a/b/target.txt");
    let revealed = tree.reveal(&target).unwrap().unwrap();

    assert_eq!(tree.node_path(revealed), Some(target.as_path()));
    assert_eq!(
        names(tree.rows()),
        vec![display_name(&dir).as_str(), "a", "b", "target.txt"]
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn reveal_outside_the_root_returns_none() {
    let dir = temp_dir();
    let mut tree = FileTree::open(&dir).unwrap();
    assert_eq!(tree.reveal("/definitely/not/in/this/tree").unwrap(), None);
    fs::remove_dir_all(&dir).ok();
}
