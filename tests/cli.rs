//! The command line, run as a binary in throwaway folders.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn codeweight(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeweight"))
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

const JAVA: &str = "class A {\n  void f() {\n    g();\n  }\n}\n";

#[test]
fn init_writes_the_defaults_and_never_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let output = codeweight(dir.path(), &["init"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let written = fs::read_to_string(dir.path().join("codeweight.toml")).unwrap();
    assert!(written.contains("[weights]"));

    let again = codeweight(dir.path(), &["init"]);
    assert_eq!(again.status.code(), Some(2));
    assert!(stderr(&again).contains("not overwritten"));

    let named = codeweight(dir.path(), &["init", "mine.toml"]);
    assert!(named.status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("mine.toml")).unwrap(),
        written
    );
}

#[test]
fn scan_reports_a_java_file_s_functions_and_classes() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("A.java"), JAVA).unwrap();
    // Under the defaults: A.f is function 2, statement 1 and call 0.5; A is class 5.
    let output = codeweight(dir.path(), &["scan", "A.java"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "8.5\n");

    let verbose = codeweight(dir.path(), &["scan", "A.java", "--verbose"]);
    assert_eq!(
        stdout(&verbose),
        "total 8.5\n  5  A.java  A (class)\n3.5  A.java  A.f\n"
    );
    let short = codeweight(dir.path(), &["scan", "-v", "A.java"]);
    assert_eq!(stdout(&short), stdout(&verbose));

    let by_file = codeweight(dir.path(), &["scan", "A.java", "--by", "file"]);
    assert_eq!(stdout(&by_file), "total 8.5\n8.5  A.java\n");
    let top = codeweight(dir.path(), &["scan", "A.java", "--top", "1"]);
    assert_eq!(stdout(&top), "total 8.5\n5  A.java  A (class)\n");
}

#[test]
fn scan_leaves_out_an_unsupported_file_and_warns_of_syntax_errors() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("notes.txt"), "hello").unwrap();
    fs::write(dir.path().join("B.java"), "class B { void f( }").unwrap();
    let output = codeweight(dir.path(), &["scan", "notes.txt", "B.java", "-v"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let out = stdout(&output);
    assert!(!out.contains("notes.txt"), "{out}");
    assert!(out.contains("warning: B.java has syntax errors"), "{out}");

    // The total alone: the warning to stderr.
    let quiet = codeweight(dir.path(), &["scan", "notes.txt", "B.java"]);
    assert!(quiet.status.success(), "{}", stderr(&quiet));
    assert_eq!(stdout(&quiet).lines().count(), 1, "{}", stdout(&quiet));
    assert!(stdout(&quiet).trim().parse::<f64>().is_ok());
    assert!(stderr(&quiet).starts_with("warning: B.java has syntax errors"));
}

#[test]
fn a_bad_path_or_weights_file_ends_with_exit_2() {
    let dir = tempfile::tempdir().unwrap();
    let missing = codeweight(dir.path(), &["scan", "Missing.java"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(stderr(&missing).starts_with("error: "));

    fs::write(dir.path().join("A.java"), JAVA).unwrap();
    fs::write(dir.path().join("bad.toml"), "[weights]\nloops = 1\n").unwrap();
    let bad = codeweight(dir.path(), &["scan", "A.java", "--config", "bad.toml"]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(
        stderr(&bad).contains("unknown category `loops`"),
        "{}",
        stderr(&bad)
    );

    let absent = codeweight(dir.path(), &["scan", "A.java", "--config", "absent.toml"]);
    assert_eq!(absent.status.code(), Some(2));
}

#[test]
fn the_weights_file_is_looked_up_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let elsewhere = dir.path().join("elsewhere");
    fs::create_dir_all(repo.join(".git")).unwrap();
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::create_dir_all(&elsewhere).unwrap();
    fs::write(repo.join("src/A.java"), JAVA).unwrap();
    let file = repo.join("src/A.java");
    let file = file.to_str().unwrap();
    let total = |cwd: &Path, extra: &[&str]| {
        let mut args = vec!["scan", file, "--by", "file"];
        args.extend_from_slice(extra);
        let output = codeweight(cwd, &args);
        assert!(output.status.success(), "{}", stderr(&output));
        stdout(&output).lines().next().unwrap().to_string()
    };

    assert_eq!(total(&elsewhere, &[]), "total 8.5");
    fs::write(repo.join("codeweight.toml"), "[weights]\nclass = 100\n").unwrap();
    assert_eq!(total(&elsewhere, &[]), "total 103.5");
    fs::write(
        elsewhere.join("codeweight.toml"),
        "[weights]\nclass = 1000\n",
    )
    .unwrap();
    assert_eq!(total(&elsewhere, &[]), "total 1003.5");
    fs::write(elsewhere.join("given.toml"), "[weights]\nfunction = 0\n").unwrap();
    assert_eq!(total(&elsewhere, &["--config", "given.toml"]), "total 6.5");
}

#[test]
fn scan_walks_a_folder_its_excludes_and_tests_left_out_and_scan_tests_weighs_the_tests() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for (file, text) in [
        // Under the defaults: f is function 2, statement 1, call 0.5.
        ("src/a.py", "def f():\n    g()\n"),
        // h is function 2, its call 0.5 and its body's statement 1; the file's declaration statement 1.
        ("src/b.ts", "export const h = () => g();\n"),
        // A test: t is 3.5.
        ("tests/test_a.py", "def t():\n    g()\n"),
        // c is 3.5, its test u 3.5 (a class 5 and a function 2 of the tests, each with a statement and a call).
        (
            "src/c.rs",
            "fn c() { g(); }\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn u() { c(); }\n}\n",
        ),
        ("node_modules/x/y.ts", "g();\n"),
        ("src/notes.md", "# notes\n"),
    ] {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    let output = codeweight(root, &["scan", "."]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "11.5\n");

    let verbose = codeweight(root, &["scan", ".", "--verbose"]);
    assert_eq!(
        stdout(&verbose),
        "total 11.5\n\
         3.5  src/a.py  f\n\
         3.5  src/b.ts  h\n\
         3.5  src/c.rs  c\n\
         \x20 1  src/b.ts  (file)\n"
    );

    let by_file = codeweight(root, &["scan", "src", "tests", "--by", "file"]);
    assert_eq!(
        stdout(&by_file),
        "total 11.5\n4.5  src/b.ts\n3.5  src/a.py\n3.5  src/c.rs\n"
    );

    let tests = codeweight(root, &["scan-tests", "."]);
    assert!(tests.status.success(), "{}", stderr(&tests));
    assert_eq!(stdout(&tests), "12\n");
    let tests = codeweight(root, &["scan-tests", ".", "-v"]);
    assert_eq!(
        stdout(&tests),
        "total 12\n\
         \x20 5  src/c.rs  tests (class)\n\
         3.5  src/c.rs  tests.u\n\
         3.5  tests/test_a.py  t\n"
    );
}

#[test]
fn scan_weighs_go_its_test_files_and_vendor_and_generated_files_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for (file, text) in [
        // Under the defaults: the method S.m is function 2, statement 1, call 0.5.
        ("pkg/a.go", "package pkg\n\nfunc (s *S) m() { g() }\n"),
        // A test file: TestM is 3.5.
        (
            "pkg/a_test.go",
            "package pkg\n\nfunc TestM(t *testing.T) { g() }\n",
        ),
        ("vendor/x/y.go", "package x\n\nfunc v() { g() }\n"),
        // Generated: left out.
        ("pkg/a.pb.go", "package pkg\n\nfunc p() { g() }\n"),
    ] {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    let output = codeweight(root, &["scan", ".", "-v"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "total 3.5\n3.5  pkg/a.go  S.m\n");
    let tests = codeweight(root, &["scan-tests", ".", "-v"]);
    assert_eq!(stdout(&tests), "total 3.5\n3.5  pkg/a_test.go  TestM\n");
}

/// A throwaway git repo, isolated from the user's and the system's git config.
struct Repo {
    dir: tempfile::TempDir,
}

impl Repo {
    fn new() -> Repo {
        let repo = Repo {
            dir: tempfile::tempdir().unwrap(),
        };
        repo.git(&["init", "-q", "-b", "main"]);
        repo
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(self.path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {}", stderr(&output));
        stdout(&output).trim().to_string()
    }

    /// Writes (a text) or deletes (none) the files, then commits them all; the new commit's hash.
    fn commit(&self, files: &[(&str, Option<&str>)]) -> String {
        for (file, text) in files {
            let path = self.path().join(file);
            match text {
                Some(text) => {
                    fs::create_dir_all(path.parent().unwrap()).unwrap();
                    fs::write(path, text).unwrap();
                }
                None => fs::remove_file(path).unwrap(),
            }
        }
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "--allow-empty", "-m", "change"]);
        self.git(&["rev-parse", "HEAD"])
    }

    /// `codeweight commit -v` (the whole report) under the engine's test weights (statement 1, call 2, function
    /// 100), its stdout.
    fn weigh(&self, args: &[&str]) -> String {
        self.run("commit", args)
    }

    /// `codeweight pre-commit -v`, the uncommitted changes, as `weigh`.
    fn pre_commit(&self, args: &[&str]) -> String {
        self.run("pre-commit", args)
    }

    fn run(&self, command: &str, args: &[&str]) -> String {
        let weights = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/weights.toml");
        let mut all = vec![command, "-v"];
        all.extend_from_slice(args);
        all.extend_from_slice(&["--config", weights]);
        let output = codeweight(self.path(), &all);
        assert!(output.status.success(), "{}", stderr(&output));
        stdout(&output)
    }
}

// Under tests/weights.toml: F's f is 103 (function 100, statement 1, call 2), FG's 106; FH's h is 100 (`pass`
// weighs nothing).
const F: &str = "def f():\n    g()\n";
const FG: &str = "def f():\n    g()\n    g()\n";
const FH: &str = "def f():\n    g()\n\n\ndef h():\n    pass\n";

#[test]
fn commit_weighs_the_root_commit_against_the_empty_tree() {
    let repo = Repo::new();
    repo.commit(&[("a.py", Some(F)), ("notes.md", Some("# notes\n"))]);
    assert_eq!(
        repo.weigh(&["HEAD"]),
        "total +103\n\
         +103  added    a.py  f\n"
    );

    // The total delta alone, without `-v`.
    let output = codeweight(repo.path(), &["commit", "HEAD"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "+3.5\n");
}

#[test]
fn commit_weighs_an_add_a_change_a_delete_and_a_rename() {
    let repo = Repo::new();
    // gone.py unlike b.py, so git doesn't take the two for a rename.
    repo.commit(&[
        ("a.py", Some(F)),
        ("gone.py", Some("def x():\n    pass\n")),
        ("old.py", Some(FH)),
    ]);
    // A long enough file for git to see the rename though a line changed: h's `pass` is dropped for `g()`.
    let renamed = format!("{FH}{}", "\n\n# a comment\n".repeat(20));
    repo.commit(&[("old.py", Some(&renamed))]);
    repo.commit(&[
        ("a.py", Some(FG)),
        ("b.py", Some(F)),
        ("gone.py", None),
        ("old.py", None),
        ("new.py", Some(&renamed.replace("    pass\n", "    g()\n"))),
    ]);
    assert_eq!(
        repo.weigh(&["HEAD"]),
        "total +9\n\
         +103  added    b.py  f\n\
         -100  removed  gone.py  x\n\
         \x20 +3  heavier  a.py  f\n\
         \x20 +3  heavier  old.py -> new.py  h\n"
    );
    assert_eq!(
        repo.weigh(&["HEAD", "--by", "file", "--top", "2"]),
        "total +9\n+103  added    b.py\n-100  removed  gone.py\n"
    );
}

#[test]
fn commit_weighs_a_merge_against_its_first_parent() {
    let repo = Repo::new();
    repo.commit(&[("a.py", Some(F))]);
    repo.git(&["checkout", "-q", "-b", "side"]);
    repo.commit(&[("b.py", Some(F))]);
    repo.git(&["checkout", "-q", "main"]);
    repo.commit(&[("a.py", Some(FG))]);
    repo.git(&["merge", "-q", "--no-ff", "--no-edit", "side"]);
    // The merge brings the side's b.py into main; main's own change to a.py is the first parent's.
    assert_eq!(
        repo.weigh(&["HEAD"]),
        "total +103\n+103  added    b.py  f\n"
    );
}

#[test]
fn commit_weighs_a_range_tree_to_tree_so_an_undone_change_nets_zero() {
    let repo = Repo::new();
    let start = repo.commit(&[("a.py", Some(F))]);
    repo.commit(&[("a.py", Some(FG)), ("b.py", Some(F))]);
    repo.commit(&[("a.py", Some(F))]);
    assert_eq!(
        repo.weigh(&[&format!("{start}..HEAD")]),
        "total +103\n+103  added    b.py  f\n"
    );
    assert_eq!(repo.weigh(&["HEAD~1..HEAD~1"]), "total 0\n");

    let json: serde_json::Value =
        serde_json::from_str(&repo.weigh(&["HEAD~2..HEAD~1", "--format", "json"])).unwrap();
    assert_eq!(json["from"], serde_json::json!(start));
    assert_eq!(json["total"], serde_json::json!(106.0));
    assert_eq!(json["rows"].as_array().unwrap().len(), 2);
}

#[test]
fn commit_leaves_out_the_excludes_and_its_tests_and_commit_tests_weighs_the_tests() {
    let repo = Repo::new();
    repo.commit(&[
        ("build/gen.py", Some(F)),
        (".hidden/h.py", Some(F)),
        ("tests/test_a.py", Some(F)),
        ("a.py", Some(F)),
        (
            "c.rs",
            Some(
                "fn c() { g(); }\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn u() { c(); }\n}\n",
            ),
        ),
    ]);
    fs::write(
        repo.path().join("own.toml"),
        "[files]\nexclude = [\"build/**\"]\n",
    )
    .unwrap();
    let run = |args: &[&str]| {
        let mut all = args.to_vec();
        all.extend_from_slice(&["-v", "--config", "own.toml"]);
        let output = codeweight(repo.path(), &all);
        assert!(output.status.success(), "{}", stderr(&output));
        stdout(&output)
    };
    // Under the defaults: f, c and u are 3.5 (function 2, statement 1, call 0.5), the class of the tests 5.
    assert_eq!(
        run(&["commit", "HEAD"]),
        "total +7\n\
         +3.5  added    a.py  f\n\
         +3.5  added    c.rs  c\n"
    );
    assert_eq!(
        run(&["commit-tests", "HEAD"]),
        "total +12\n\
         \x20 +5  added    c.rs  tests (class)\n\
         +3.5  added    c.rs  tests.u\n\
         +3.5  added    tests/test_a.py  f\n"
    );

    // The uncommitted changes: a test heavier by a statement and a call.
    fs::write(repo.path().join("tests/test_a.py"), FG).unwrap();
    assert_eq!(run(&["pre-commit"]), "total 0\n");
    assert_eq!(
        run(&["pre-commit-tests"]),
        "total +1.5\n+1.5  heavier  tests/test_a.py  f\n"
    );
}

#[test]
fn pre_commit_weighs_the_uncommitted_changes_and_leaves_the_index_alone() {
    let repo = Repo::new();
    repo.commit(&[
        ("a.py", Some(F)),
        ("gone.py", Some("def x():\n    pass\n")),
        (".gitignore", Some("ignored.py\n")),
    ]);
    assert_eq!(repo.pre_commit(&[]), "total 0\n");

    // One change staged, the others not, one file untracked, one ignored.
    fs::write(repo.path().join("staged.py"), F).unwrap();
    repo.git(&["add", "staged.py"]);
    fs::write(repo.path().join("a.py"), FG).unwrap();
    fs::write(repo.path().join("untracked.py"), F).unwrap();
    fs::write(repo.path().join("ignored.py"), F).unwrap();
    fs::remove_file(repo.path().join("gone.py")).unwrap();
    let state = || {
        [
            repo.git(&["ls-files", "-s"]),
            repo.git(&["status", "--porcelain"]),
        ]
    };
    let before = state();
    assert_eq!(
        repo.pre_commit(&[]),
        "total +109\n\
         +103  added    staged.py  f\n\
         +103  added    untracked.py  f\n\
         -100  removed  gone.py  x\n\
         \x20 +3  heavier  a.py  f\n"
    );
    assert_eq!(state(), before);
    // The staged change alone.
    assert_eq!(
        repo.pre_commit(&["--only-staged"]),
        "total +103\n+103  added    staged.py  f\n"
    );
    assert_eq!(state(), before);

    let json: serde_json::Value =
        serde_json::from_str(&repo.pre_commit(&["--format", "json"])).unwrap();
    assert_eq!(
        json["from"],
        serde_json::json!(repo.git(&["rev-parse", "HEAD"]))
    );
    assert_eq!(json["to"], serde_json::Value::Null);
    assert_eq!(json["total"], serde_json::json!(109.0));
}

#[test]
fn pre_commit_sees_a_rename_into_an_untracked_file_and_a_repo_with_no_commit() {
    let repo = Repo::new();
    let renamed = format!("{FH}{}", "\n\n# a comment\n".repeat(20));
    repo.commit(&[("old.py", Some(&renamed))]);
    fs::remove_file(repo.path().join("old.py")).unwrap();
    fs::write(
        repo.path().join("new.py"),
        renamed.replace("    pass\n", "    g()\n"),
    )
    .unwrap();
    assert_eq!(
        repo.pre_commit(&[]),
        "total +3\n+3  heavier  old.py -> new.py  h\n"
    );

    // No commit yet: against the empty tree.
    let fresh = Repo::new();
    fs::write(fresh.path().join("a.py"), F).unwrap();
    assert_eq!(
        fresh.pre_commit(&[]),
        "total +103\n+103  added    a.py  f\n"
    );
    // Nothing staged yet (no index at all), then the file staged.
    assert_eq!(fresh.pre_commit(&["--only-staged"]), "total 0\n");
    fresh.git(&["add", "a.py"]);
    assert_eq!(
        fresh.pre_commit(&["--only-staged"]),
        "total +103\n+103  added    a.py  f\n"
    );
    let json: serde_json::Value =
        serde_json::from_str(&fresh.pre_commit(&["--format", "json"])).unwrap();
    assert_eq!(json["from"], serde_json::Value::Null);
    assert_eq!(json["to"], serde_json::Value::Null);
}

// Linux only: a file's name there may be any bytes, where macOS's file systems and Windows want UTF-8.
#[cfg(target_os = "linux")]
#[test]
fn commit_weighs_a_file_whose_name_isn_t_utf8() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let repo = Repo::new();
    fs::write(repo.path().join(OsStr::from_bytes(b"caf\xe9.py")), F).unwrap();
    repo.commit(&[("a.py", Some(FG))]);
    assert_eq!(
        repo.weigh(&["HEAD"]),
        "total +209\n\
         +106  added    a.py  f\n\
         +103  added    caf\u{fffd}.py  f\n"
    );
}

#[test]
fn commit_at_a_shallow_clone_s_boundary_is_an_error_not_a_root() {
    let repo = Repo::new();
    repo.commit(&[("a.py", Some(F))]);
    repo.commit(&[("a.py", Some(FG))]);
    let clone = Repo::new();
    let url = format!("file://{}", repo.path().display());
    clone.git(&["fetch", "-q", "--depth", "1", &url, "main"]);
    clone.git(&["reset", "-q", "--hard", "FETCH_HEAD"]);
    let output = codeweight(clone.path(), &["commit", "HEAD"]);
    assert_eq!(output.status.code(), Some(2), "{}", stdout(&output));
    assert!(
        stderr(&output).contains("shallow clone"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn commit_of_a_bad_revision_or_outside_a_repo_ends_with_exit_2() {
    let repo = Repo::new();
    repo.commit(&[("a.py", Some(F))]);
    let bad = codeweight(repo.path(), &["commit", "nope"]);
    assert_eq!(bad.status.code(), Some(2));
    assert!(
        stderr(&bad).contains("`nope` is not a commit"),
        "{}",
        stderr(&bad)
    );
    let outside = tempfile::tempdir().unwrap();
    let none = codeweight(outside.path(), &["commit", "HEAD"]);
    assert_eq!(none.status.code(), Some(2));
}

#[test]
fn scan_s_json_lists_the_rows_of_its_text() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("A.java"), JAVA).unwrap();
    fs::write(dir.path().join("notes.txt"), "hello").unwrap();
    let output = codeweight(
        dir.path(),
        &[
            "scan",
            "A.java",
            "notes.txt",
            "--format",
            "json",
            "--top",
            "1",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    // Under the defaults, as in the text: A is 5, A.f 3.5.
    assert_eq!(
        json,
        serde_json::json!({
            "total": 8.5,
            "by": "function",
            "rows": [{"path": "A.java", "kind": "class", "name": "A", "line": 1, "weight": 5.0}],
            "warnings": []
        })
    );
}
