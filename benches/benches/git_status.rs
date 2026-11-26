//! ST-SIDE-03: the tree's git status reruns on every save, git operation and
//! watcher batch, so its cost on a large repository is what decides whether
//! "live" holds. This measures `lux_git::diff_panel::worktree_status` on a
//! generated repository of 10,000 tracked files (100 directories x 100
//! files, ~2 KB each) plus one untracked directory of 500 files:
//!
//! - `stat_clean`: every tracked file matches its index stat; nothing is
//!   read, only stat'ed — the common case after a save.
//! - `one_file_edited`: one tracked file is stat-dirty and is read.
//! - `no_stat_cache`: the index carries no stat data, so every tracked file
//!   is read and compared, as before the stat cache existed.

use std::fs;
use std::hint::black_box;
use std::path::Path;
use std::time::{Duration, SystemTime};

use criterion::{Criterion, criterion_group, criterion_main};
use lux_git::gix;

const DIRS: usize = 100;
const FILES_PER_DIR: usize = 100;
const UNTRACKED: usize = 500;

/// A repository with every file committed and staged. `with_stat` decides
/// whether the index records each file's stat (as `git add` does) or
/// leaves it empty.
fn make_repo(root: &Path, with_stat: bool) -> gix::Repository {
    let repo = gix::init(root).expect("init");
    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000);
    let mut state = gix::index::State::new(repo.object_hash());
    let body = "fn line() -> usize { 42 }\n".repeat(80);
    for d in 0..DIRS {
        fs::create_dir_all(root.join(format!("src/m{d:03}"))).unwrap();
        for f in 0..FILES_PER_DIR {
            let rel = format!("src/m{d:03}/f{f:03}.rs");
            let absolute = root.join(&rel);
            let content = format!("// {rel}\n{body}");
            fs::write(&absolute, &content).unwrap();
            // Older than the index, so its stat is not racy.
            let file = fs::OpenOptions::new().write(true).open(&absolute).unwrap();
            file.set_modified(old).unwrap();
            drop(file);
            let id = repo.write_blob(content.as_bytes()).unwrap().detach();
            let stat = if with_stat {
                let metadata = gix::index::fs::Metadata::from_path_no_follow(&absolute).unwrap();
                gix::index::entry::Stat::from_fs(&metadata).unwrap()
            } else {
                gix::index::entry::Stat::default()
            };
            state.dangerously_push_entry(
                stat,
                id,
                gix::index::entry::Flags::empty(),
                gix::index::entry::Mode::FILE,
                rel.as_str().into(),
            );
        }
    }
    state.sort_entries();
    let mut index = gix::index::File::from_state(state, repo.index_path());
    index
        .write(gix::index::write::Options::default())
        .expect("write index");
    lux_git::commit::commit(&repo, "root", "Lux Bench", "bench@example.com").expect("commit");
    fs::create_dir_all(root.join("scratch")).unwrap();
    for f in 0..UNTRACKED {
        fs::write(root.join(format!("scratch/u{f:03}.txt")), "untracked\n").unwrap();
    }
    gix::open(root).expect("reopen")
}

fn bench_git_status(c: &mut Criterion) {
    let mut group = c.benchmark_group("git_status_10k");
    group.sample_size(20);

    let clean_dir = tempfile::tempdir().unwrap();
    let clean = make_repo(clean_dir.path(), true);
    let status = lux_git::diff_panel::worktree_status(&clean).unwrap();
    assert_eq!(status.untracked_dirs.len(), 1, "{:?}", status.changes);
    assert_eq!(status.changes.len(), 1, "{:?}", status.changes);
    group.bench_function("stat_clean", |b| {
        b.iter(|| black_box(lux_git::diff_panel::worktree_status(&clean).unwrap()))
    });

    fs::write(clean_dir.path().join("src/m050/f050.rs"), "edited\n").unwrap();
    group.bench_function("one_file_edited", |b| {
        b.iter(|| black_box(lux_git::diff_panel::worktree_status(&clean).unwrap()))
    });

    let cold_dir = tempfile::tempdir().unwrap();
    let cold = make_repo(cold_dir.path(), false);
    group.bench_function("no_stat_cache", |b| {
        b.iter(|| black_box(lux_git::diff_panel::worktree_status(&cold).unwrap()))
    });
    group.finish();
}

criterion_group!(benches, bench_git_status);
criterion_main!(benches);
