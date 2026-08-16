use std::{
    collections::BTreeSet,
    error::Error,
    fmt,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher, WatcherKind};
use walkdir::WalkDir;

const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub const DEFAULT_IGNORE_PATTERNS: &[&str] =
    &[".git", "__pycache__", ".pytest_cache", ".venv", "target"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchBatch {
    paths: Vec<PathBuf>,
}

impl WatchBatch {
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    pub fn into_paths(self) -> Vec<PathBuf> {
        self.paths
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchError {
    message: String,
}

impl WatchError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for WatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for WatchError {}

/// Interpreter-neutral recursive Python source watcher.
///
/// The worker owns only filesystem paths and synchronization primitives. It
/// never enters an interpreter and never touches a Bevy `World`.
pub struct FileWatcher {
    batches: Mutex<Receiver<Result<WatchBatch, WatchError>>>,
    stop_sender: Mutex<Option<Sender<()>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl FileWatcher {
    pub fn start(
        roots: Vec<PathBuf>,
        ignore_patterns: Vec<String>,
        debounce: Duration,
    ) -> Result<Self, WatchError> {
        if roots.is_empty() {
            return Err(WatchError::new("at least one watch path is required"));
        }

        let roots = roots
            .into_iter()
            .map(|path| absolute_path(&path))
            .collect::<Vec<_>>();
        let (batch_sender, batch_receiver) = mpsc::channel();
        let (stop_sender, stop_receiver) = mpsc::channel();
        let (startup_sender, startup_receiver) = mpsc::sync_channel(1);

        let worker = thread::Builder::new()
            .name("pybevy-file-watcher".to_string())
            .spawn(move || {
                watch_worker(
                    roots,
                    ignore_patterns,
                    debounce,
                    batch_sender,
                    stop_receiver,
                    startup_sender,
                );
            })
            .map_err(|error| {
                WatchError::new(format!("failed to spawn file watcher thread: {error}"))
            })?;

        match startup_receiver.recv() {
            Ok(Ok(())) => Ok(Self {
                batches: Mutex::new(batch_receiver),
                stop_sender: Mutex::new(Some(stop_sender)),
                worker: Mutex::new(Some(worker)),
            }),
            Ok(Err(error)) => {
                let _ = worker.join();
                Err(error)
            }
            Err(_) => {
                let _ = worker.join();
                Err(WatchError::new(
                    "file watcher stopped before initialization completed",
                ))
            }
        }
    }

    pub fn with_defaults(roots: Vec<PathBuf>) -> Result<Self, WatchError> {
        Self::start(
            roots,
            DEFAULT_IGNORE_PATTERNS
                .iter()
                .map(|pattern| (*pattern).to_string())
                .collect(),
            Duration::from_millis(50),
        )
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<Option<WatchBatch>, WatchError> {
        match lock_or_recover(&self.batches).recv_timeout(timeout) {
            Ok(result) => result.map(Some),
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => Ok(None),
        }
    }

    pub fn try_recv(&self) -> Result<Option<WatchBatch>, WatchError> {
        match lock_or_recover(&self.batches).try_recv() {
            Ok(result) => result.map(Some),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => Ok(None),
        }
    }

    pub fn stop(&self) -> Result<(), WatchError> {
        if let Some(sender) = lock_or_recover(&self.stop_sender).take() {
            let _ = sender.send(());
        }
        if let Some(worker) = lock_or_recover(&self.worker).take() {
            worker
                .join()
                .map_err(|_| WatchError::new("file watcher thread panicked"))?;
        }
        Ok(())
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn watch_worker(
    roots: Vec<PathBuf>,
    ignore_patterns: Vec<String>,
    debounce: Duration,
    batch_sender: Sender<Result<WatchBatch, WatchError>>,
    stop_receiver: Receiver<()>,
    startup_sender: mpsc::SyncSender<Result<(), WatchError>>,
) {
    let (event_sender, event_receiver) = mpsc::channel();
    let mut watcher = match notify::recommended_watcher(move |result| {
        let _ = event_sender.send(result);
    }) {
        Ok(watcher) => watcher,
        Err(error) => {
            let _ = startup_sender.send(Err(WatchError::new(format!(
                "failed to create file watcher: {error}"
            ))));
            return;
        }
    };

    // notify implements recursive inotify watches by walking every descendant before
    // registering one watch per directory. Build that plan here so ignored subtrees
    // are pruned before either the traversal or the watch registration happens.
    let prune_recursive_watches = <RecommendedWatcher as Watcher>::kind() == WatcherKind::Inotify;
    let mut watched_paths = BTreeSet::new();

    for root in &roots {
        let watch_result = if prune_recursive_watches {
            let outcome = watch_pruned_tree(
                &mut watcher,
                root,
                &ignore_patterns,
                &mut watched_paths,
                false,
            );
            watch_errors_result(outcome.errors)
        } else {
            watcher
                .watch(root, RecursiveMode::Recursive)
                .map_err(|error| WatchError::new(error.to_string()))
        };
        if let Err(error) = watch_result {
            let _ = startup_sender.send(Err(WatchError::new(format!(
                "failed to watch '{}': {error}",
                root.display()
            ))));
            return;
        }
    }

    if startup_sender.send(Ok(())).is_err() {
        return;
    }

    loop {
        if stop_receiver.try_recv().is_ok() {
            return;
        }

        let first_event = match event_receiver.recv_timeout(EVENT_POLL_INTERVAL) {
            Ok(Ok(event)) => event,
            Ok(Err(error)) => {
                if batch_sender
                    .send(Err(WatchError::new(format!(
                        "file watcher event error: {error}"
                    ))))
                    .is_err()
                {
                    return;
                }
                continue;
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        };

        let mut paths = BTreeSet::new();
        if prune_recursive_watches {
            handle_directory_event(
                &mut watcher,
                &first_event,
                &ignore_patterns,
                &mut watched_paths,
                &mut paths,
                &batch_sender,
            );
        }
        collect_event_paths(&first_event, &ignore_patterns, &mut paths);
        let deadline = Instant::now() + debounce;

        while Instant::now() < deadline {
            if stop_receiver.try_recv().is_ok() {
                return;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            match event_receiver.recv_timeout(remaining.min(EVENT_POLL_INTERVAL)) {
                Ok(Ok(event)) => {
                    if prune_recursive_watches {
                        handle_directory_event(
                            &mut watcher,
                            &event,
                            &ignore_patterns,
                            &mut watched_paths,
                            &mut paths,
                            &batch_sender,
                        );
                    }
                    collect_event_paths(&event, &ignore_patterns, &mut paths);
                }
                Ok(Err(error)) => {
                    if batch_sender
                        .send(Err(WatchError::new(format!(
                            "file watcher event error: {error}"
                        ))))
                        .is_err()
                    {
                        return;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }

        if !paths.is_empty()
            && batch_sender
                .send(Ok(WatchBatch {
                    paths: paths.into_iter().collect(),
                }))
                .is_err()
        {
            return;
        }
    }
}

struct WatchTreeOutcome {
    python_files: Vec<PathBuf>,
    errors: Vec<WatchError>,
}

fn watch_pruned_tree(
    watcher: &mut RecommendedWatcher,
    root: &Path,
    ignore_patterns: &[String],
    watched_paths: &mut BTreeSet<PathBuf>,
    discover_python_files: bool,
) -> WatchTreeOutcome {
    let (paths_to_watch, python_files) =
        collect_pruned_tree(root, ignore_patterns, discover_python_files);

    let errors = register_watch_paths(paths_to_watch, watched_paths, |path| {
        watcher
            .watch(path, RecursiveMode::NonRecursive)
            .map_err(|error| {
                WatchError::new(format!("failed to watch '{}': {error}", path.display()))
            })
    });

    WatchTreeOutcome {
        python_files,
        errors,
    }
}

fn register_watch_paths(
    paths_to_watch: Vec<PathBuf>,
    watched_paths: &mut BTreeSet<PathBuf>,
    mut register: impl FnMut(&Path) -> Result<(), WatchError>,
) -> Vec<WatchError> {
    let mut errors = Vec::new();
    for path in paths_to_watch {
        let absolute = absolute_path(&path);
        if watched_paths.contains(&absolute) {
            continue;
        }
        match register(&absolute) {
            Ok(()) => {
                watched_paths.insert(absolute);
            }
            Err(error) => errors.push(error),
        }
    }
    errors
}

fn watch_errors_result(errors: Vec<WatchError>) -> Result<(), WatchError> {
    if errors.is_empty() {
        return Ok(());
    }

    Err(WatchError::new(
        errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("; "),
    ))
}

fn collect_pruned_tree(
    root: &Path,
    ignore_patterns: &[String],
    discover_python_files: bool,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    if is_ignored_path(root, ignore_patterns) {
        return (Vec::new(), Vec::new());
    }
    if !root.is_dir() {
        return (vec![root.to_path_buf()], Vec::new());
    }

    let mut paths_to_watch = Vec::new();
    let mut python_files = Vec::new();
    for entry in WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_entry(|entry| !is_ignored_path(entry.path(), ignore_patterns))
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if entry.metadata().is_ok_and(|metadata| metadata.is_dir()) {
            paths_to_watch.push(path.to_path_buf());
        } else if discover_python_files && is_python_path(path, ignore_patterns) {
            python_files.push(absolute_path(path));
        }
    }

    (paths_to_watch, python_files)
}

fn handle_directory_event(
    watcher: &mut RecommendedWatcher,
    event: &Event,
    ignore_patterns: &[String],
    watched_paths: &mut BTreeSet<PathBuf>,
    paths: &mut BTreeSet<PathBuf>,
    batch_sender: &Sender<Result<WatchBatch, WatchError>>,
) {
    if !should_scan_directory_tree(&event.kind) {
        forget_removed_watch_paths(event, watched_paths);
        return;
    }

    for path in &event.paths {
        if path.is_dir() && !is_ignored_path(path, ignore_patterns) {
            let outcome = watch_pruned_tree(watcher, path, ignore_patterns, watched_paths, true);
            paths.extend(outcome.python_files);
            for error in outcome.errors {
                let _ = batch_sender.send(Err(error));
            }
        }
    }
    forget_removed_watch_paths(event, watched_paths);
}

fn should_scan_directory_tree(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(notify::event::ModifyKind::Name(_))
    )
}

fn forget_removed_watch_paths(event: &Event, watched_paths: &mut BTreeSet<PathBuf>) {
    if !matches!(&event.kind, EventKind::Remove(_) | EventKind::Modify(_)) {
        return;
    }

    for path in &event.paths {
        if path.exists() {
            continue;
        }
        let absolute = absolute_path(path);
        watched_paths.retain(|watched| !watched.starts_with(&absolute));
    }
}

fn collect_event_paths(event: &Event, ignore_patterns: &[String], paths: &mut BTreeSet<PathBuf>) {
    if !matches!(
        &event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) {
        return;
    }

    for path in &event.paths {
        if is_python_path(path, ignore_patterns) {
            paths.insert(absolute_path(path));
        }
    }
}

fn is_python_path(path: &Path, ignore_patterns: &[String]) -> bool {
    if path.extension().is_none_or(|extension| extension != "py") {
        return false;
    }
    !is_ignored_path(path, ignore_patterns)
}

fn is_ignored_path(path: &Path, ignore_patterns: &[String]) -> bool {
    let path_text = path.to_string_lossy();
    ignore_patterns
        .iter()
        .any(|pattern| path_text.contains(pattern))
}

fn absolute_path(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

fn lock_or_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use notify::event::{CreateKind, ModifyKind, RemoveKind, RenameMode};

    use super::*;

    static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TempDirectory(PathBuf);

    impl TempDirectory {
        fn new() -> Self {
            let sequence = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pybevy-file-watcher-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn filters_non_python_and_ignored_paths() {
        let ignored = DEFAULT_IGNORE_PATTERNS
            .iter()
            .map(|pattern| (*pattern).to_string())
            .collect::<Vec<_>>();
        assert!(is_python_path(Path::new("src/scene.py"), &ignored));
        assert!(!is_python_path(Path::new("src/scene.txt"), &ignored));
        assert!(!is_python_path(
            Path::new("src/__pycache__/scene.py"),
            &ignored
        ));
        assert!(!is_python_path(Path::new("target/scene.py"), &ignored));
    }

    #[test]
    fn pruned_watch_plan_excludes_ignored_subtrees() {
        let directory = TempDirectory::new();
        let source_directory = directory.0.join("src/nested");
        let target_directory = directory.0.join("target/deep/cache");
        let git_directory = directory.0.join(".git/objects");
        fs::create_dir_all(&source_directory).unwrap();
        fs::create_dir_all(&target_directory).unwrap();
        fs::create_dir_all(&git_directory).unwrap();
        let source_path = source_directory.join("scene.py");
        fs::write(&source_path, "value = 1\n").unwrap();
        fs::write(target_directory.join("generated.py"), "value = 2\n").unwrap();
        fs::write(git_directory.join("hook.py"), "value = 3\n").unwrap();

        let ignored = DEFAULT_IGNORE_PATTERNS
            .iter()
            .map(|pattern| (*pattern).to_string())
            .collect::<Vec<_>>();
        let (watched_paths, python_files) = collect_pruned_tree(&directory.0, &ignored, true);

        assert!(watched_paths.contains(&directory.0));
        assert!(watched_paths.contains(&directory.0.join("src")));
        assert!(watched_paths.contains(&source_directory));
        assert!(
            watched_paths
                .iter()
                .all(|path| !path.starts_with(directory.0.join("target")))
        );
        assert!(
            watched_paths
                .iter()
                .all(|path| !path.starts_with(directory.0.join(".git")))
        );
        assert_eq!(python_files, vec![absolute_path(&source_path)]);
    }

    #[test]
    fn watch_registration_continues_after_one_path_fails() {
        let directory = TempDirectory::new();
        let first_path = directory.0.join("first");
        let failed_path = directory.0.join("vanished");
        let last_path = directory.0.join("last");
        let mut watched_paths = BTreeSet::new();
        let mut attempted_paths = Vec::new();

        let errors = register_watch_paths(
            vec![first_path.clone(), failed_path.clone(), last_path.clone()],
            &mut watched_paths,
            |path| {
                attempted_paths.push(path.to_path_buf());
                if path == failed_path.as_path() {
                    Err(WatchError::new("directory vanished"))
                } else {
                    Ok(())
                }
            },
        );

        assert_eq!(
            attempted_paths,
            vec![first_path.clone(), failed_path.clone(), last_path.clone()]
        );
        assert_eq!(errors, vec![WatchError::new("directory vanished")]);
        assert_eq!(watched_paths, BTreeSet::from([first_path, last_path]));
        assert!(!watched_paths.contains(&failed_path));
    }

    #[test]
    fn accepts_create_modify_remove_and_rename_events() {
        let ignored = Vec::new();
        let event_kinds = [
            EventKind::Create(CreateKind::File),
            EventKind::Modify(ModifyKind::Data(notify::event::DataChange::Any)),
            EventKind::Remove(RemoveKind::File),
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
        ];

        for kind in event_kinds {
            let event = Event::new(kind).add_path(PathBuf::from("scene.py"));
            let mut paths = BTreeSet::new();
            collect_event_paths(&event, &ignored, &mut paths);
            assert_eq!(paths.len(), 1);
        }
    }

    #[test]
    fn directory_tree_scan_ignores_metadata_and_content_modifications() {
        assert!(should_scan_directory_tree(&EventKind::Create(
            CreateKind::Folder
        )));
        assert!(should_scan_directory_tree(&EventKind::Modify(
            ModifyKind::Name(RenameMode::To)
        )));
        assert!(!should_scan_directory_tree(&EventKind::Modify(
            ModifyKind::Metadata(notify::event::MetadataKind::Permissions)
        )));
        assert!(!should_scan_directory_tree(&EventKind::Modify(
            ModifyKind::Data(notify::event::DataChange::Any)
        )));
    }

    #[test]
    fn reports_absolute_python_paths() {
        let directory = TempDirectory::new();
        let watcher = FileWatcher::with_defaults(vec![directory.0.clone()]).unwrap();
        let source_path = directory.0.join("scene.py");
        fs::write(&source_path, "value = 1\n").unwrap();

        let batch = watcher
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .expect("watcher did not report the created Python file");

        assert!(batch.paths().contains(&absolute_path(&source_path)));
        watcher.stop().unwrap();
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn reports_python_files_already_present_in_a_moved_directory_tree() {
        let watched_directory = TempDirectory::new();
        let staging_directory = TempDirectory::new();
        let staged_tree = staging_directory.0.join("generated/nested");
        fs::create_dir_all(&staged_tree).unwrap();
        let staged_source = staged_tree.join("scene.py");
        fs::write(&staged_source, "value = 1\n").unwrap();

        let watcher = FileWatcher::with_defaults(vec![watched_directory.0.clone()]).unwrap();
        let moved_tree = watched_directory.0.join("generated");
        fs::rename(staging_directory.0.join("generated"), &moved_tree).unwrap();
        let moved_source = moved_tree.join("nested/scene.py");

        let batch = watcher
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .expect("watcher did not report Python files in the moved directory tree");

        assert!(batch.paths().contains(&absolute_path(&moved_source)));
        watcher.stop().unwrap();
    }

    #[test]
    fn stop_is_idempotent() {
        let directory = TempDirectory::new();
        let watcher = FileWatcher::with_defaults(vec![directory.0.clone()]).unwrap();
        watcher.stop().unwrap();
        watcher.stop().unwrap();
        assert_eq!(
            watcher.recv_timeout(Duration::from_millis(10)).unwrap(),
            None
        );
    }
}
