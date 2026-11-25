//! The command line, run as a binary in throwaway folders.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Output, Stdio};

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
/// A weights file of the docs' weights alone, each run 1: a scan under the defaults' code weights whose docs stay
/// as worked out by hand, whatever the defaults' tuning.
const DOCS_ONES: &str = "[docs]\nword = 1\nfunction_word = 1\npunctuation = 1\nsymbol = 1\n";

#[test]
fn init_writes_the_defaults_and_never_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let output = codeweight(dir.path(), &["init"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let written = fs::read_to_string(dir.path().join("codeweight.toml")).unwrap();
    assert!(written.contains("[weights]"));

    let again = codeweight(dir.path(), &["init"]);
    assert_eq!(again.status.code(), Some(1));
    assert!(stderr(&again).contains("not overwritten"));

    let named = codeweight(dir.path(), &["init", "mine.toml"]);
    assert!(named.status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("mine.toml")).unwrap(),
        written
    );

    // `-` is stdout, no file named `-`.
    let printed = codeweight(dir.path(), &["init", "-"]);
    assert!(printed.status.success(), "{}", stderr(&printed));
    assert_eq!(stdout(&printed), written);
    assert!(!dir.path().join("-").exists());

    // A weights file is no input of `init`'s: `--config` is a usage error, before the command or after.
    for args in [
        &["--config", "x.toml", "init", "out.toml"][..],
        &["init", "out.toml", "--config", "x.toml"],
    ] {
        let output = codeweight(dir.path(), args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(stderr(&output).starts_with("error: "), "{args:?}");
        assert!(!dir.path().join("out.toml").exists(), "{args:?}");
    }
}

#[test]
fn scan_reports_a_java_file_s_functions_and_classes() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("A.java"), JAVA).unwrap();
    // Under the defaults: A.f is function 1.55, statement 1 and call 0.49; A is class 5.
    let output = codeweight(dir.path(), &["scan", "--code", "A.java"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "8.04\n");

    let verbose = codeweight(dir.path(), &["scan", "--code", "A.java", "--verbose"]);
    assert_eq!(
        stdout(&verbose),
        "total 8.04\n   5  A.java  A (class)\n3.04  A.java  A.f\n"
    );
    let short = codeweight(dir.path(), &["scan", "--code", "-v", "A.java"]);
    assert_eq!(stdout(&short), stdout(&verbose));

    let by_file = codeweight(dir.path(), &["scan", "--code", "A.java", "--by", "file"]);
    assert_eq!(stdout(&by_file), "total 8.04\n8.04  A.java\n");
    let top = codeweight(dir.path(), &["scan", "--code", "A.java", "--top", "1"]);
    assert_eq!(stdout(&top), "total 8.04\n5  A.java  A (class)\n");
}

#[test]
fn scan_leaves_out_an_unsupported_file_and_warns_of_syntax_errors() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("notes.txt"), "hello").unwrap();
    fs::write(dir.path().join("B.java"), "class B { void f( }").unwrap();
    let output = codeweight(dir.path(), &["scan", "--code", "notes.txt", "B.java", "-v"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let out = stdout(&output);
    assert!(!out.contains("notes.txt"), "{out}");
    assert!(out.contains("warning: B.java has syntax errors"), "{out}");

    // The total alone: no warning.
    let quiet = codeweight(dir.path(), &["scan", "--code", "notes.txt", "B.java"]);
    assert!(quiet.status.success(), "{}", stderr(&quiet));
    assert_eq!(stdout(&quiet).lines().count(), 1, "{}", stdout(&quiet));
    assert!(stdout(&quiet).trim().parse::<f64>().is_ok());
    assert_eq!(stderr(&quiet), "");
}

#[test]
fn a_bad_path_or_weights_file_ends_with_exit_1() {
    let dir = tempfile::tempdir().unwrap();
    let missing = codeweight(dir.path(), &["scan", "Missing.java"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(stderr(&missing).starts_with("error: "));

    fs::write(dir.path().join("A.java"), JAVA).unwrap();
    fs::write(dir.path().join("bad.toml"), "[weights]\nloops = 1\n").unwrap();
    let bad = codeweight(dir.path(), &["scan", "A.java", "--config", "bad.toml"]);
    assert_eq!(bad.status.code(), Some(1));
    assert!(
        stderr(&bad).contains("unknown category `loops`"),
        "{}",
        stderr(&bad)
    );

    let absent = codeweight(dir.path(), &["scan", "A.java", "--config", "absent.toml"]);
    assert_eq!(absent.status.code(), Some(1));
}

#[test]
fn a_reader_that_quits_early_ends_it_quietly_with_exit_0() {
    let dir = tempfile::tempdir().unwrap();
    // A report past a pipe's buffer, so it is still writing when the reader quits.
    let methods: String = (0..5000)
        .map(|i| format!("  void method_with_a_long_name_{i}() {{ g(); }}\n"))
        .collect();
    fs::write(
        dir.path().join("A.java"),
        format!("class A {{\n{methods}}}\n"),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_codeweight"))
        .current_dir(dir.path())
        .args(["scan", "--code", "-v", "A.java"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut start = [0; 5];
    stdout.read_exact(&mut start).unwrap();
    assert_eq!(&start, b"total");
    drop(stdout);
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stderr(&output), "");

    // The warnings to a closed stderr the same.
    fs::write(dir.path().join("B.java"), "class B { void f( }\n").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_codeweight"))
        .current_dir(dir.path())
        .args(["scan", "-v", "B.java"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stderr.take());
    assert_eq!(child.wait().unwrap().code(), Some(0));
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
        let mut args = vec!["scan", "--code", file, "--by", "file"];
        args.extend_from_slice(extra);
        let output = codeweight(cwd, &args);
        assert!(output.status.success(), "{}", stderr(&output));
        stdout(&output).lines().next().unwrap().to_string()
    };

    assert_eq!(total(&elsewhere, &[]), "total 8.04");
    fs::write(repo.join("codeweight.toml"), "[weights]\nclass = 100\n").unwrap();
    assert_eq!(total(&elsewhere, &[]), "total 103.04");
    fs::write(
        elsewhere.join("codeweight.toml"),
        "[weights]\nclass = 1000\n",
    )
    .unwrap();
    assert_eq!(total(&elsewhere, &[]), "total 1003.04");
    fs::write(elsewhere.join("given.toml"), "[weights]\nfunction = 0\n").unwrap();
    assert_eq!(total(&elsewhere, &["--config", "given.toml"]), "total 6.49");
}

#[test]
fn scan_walks_a_folder_its_excludes_and_tests_left_out_and_scan_tests_weighs_the_tests() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for (file, text) in [
        // Under the defaults: f is function 1.55, statement 1, call 0.49.
        ("src/a.py", "def f():\n    g()\n"),
        // h is function 1.55, its call 0.49 and its body's statement 1; the file's declaration statement 1.
        ("src/b.ts", "export const h = () => g();\n"),
        // A test: t is 3.04.
        ("tests/test_a.py", "def t():\n    g()\n"),
        // c is 3.04, its test u 3.04 (a class 5 and a function 1.55 of the tests, each with a statement and a call).
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

    let output = codeweight(root, &["scan", "--code", "."]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "10.12\n");
    // No path is `.`.
    assert_eq!(stdout(&codeweight(root, &["scan", "--code"])), "10.12\n");

    let verbose = codeweight(root, &["scan", "--code", ".", "--verbose"]);
    assert_eq!(
        stdout(&verbose),
        "total 10.12\n\
         3.04  src/a.py  f\n\
         3.04  src/b.ts  h\n\
         3.04  src/c.rs  c\n\
         \x20  1  src/b.ts  (file)\n"
    );

    let by_file = codeweight(root, &["scan", "--code", "src", "tests", "--by", "file"]);
    assert_eq!(
        stdout(&by_file),
        "total 10.12\n4.04  src/b.ts\n3.04  src/a.py\n3.04  src/c.rs\n"
    );

    let tests = codeweight(root, &["scan", "--tests", "."]);
    assert!(tests.status.success(), "{}", stderr(&tests));
    assert_eq!(stdout(&tests), "11.08\n");
    let tests = codeweight(root, &["scan", "--tests", ".", "-v"]);
    assert_eq!(
        stdout(&tests),
        "total 11.08\n\
         \x20  5  src/c.rs  tests (class)\n\
         3.04  src/c.rs  tests.u\n\
         3.04  tests/test_a.py  t\n"
    );
}

#[test]
fn scan_weighs_go_its_test_files_and_vendor_and_generated_files_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for (file, text) in [
        // Under the defaults: the method S.m is function 1.55, statement 1, call 0.49 (its receiver no parameter).
        ("pkg/a.go", "package pkg\n\nfunc (s *S) m() { g() }\n"),
        // A test file: TestM is 3.04 and its parameter 1.65.
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

    let output = codeweight(root, &["scan", "--code", ".", "-v"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "total 3.04\n3.04  pkg/a.go  S.m\n");
    let tests = codeweight(root, &["scan", "--tests", ".", "-v"]);
    assert_eq!(stdout(&tests), "total 4.69\n4.69  pkg/a_test.go  TestM\n");
}

#[test]
fn scan_weighs_c_its_headers_and_its_test_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for (file, text) in [
        // Under the defaults: m is function 1.55, statement 1, call 0.49 (`void` no parameter).
        ("src/a.c", "void m(void) { g(); }\n"),
        // A header's prototype: function 1.55, its declaration no statement.
        (
            "src/a.h",
            "#ifndef A_H\n#define A_H\nvoid n(void);\n#endif\n",
        ),
        // A test file: 3.04.
        ("src/test_a.c", "void test_m(void) { g(); }\n"),
    ] {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    let output = codeweight(root, &["scan", "--code", ".", "-v"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "total 4.59\n3.04  src/a.c  m\n1.55  src/a.h  n\n"
    );
    let tests = codeweight(root, &["scan", "--tests", ".", "-v"]);
    assert_eq!(stdout(&tests), "total 3.04\n3.04  src/test_a.c  test_m\n");
}

#[test]
fn scan_docs_weighs_the_markdown_files_alone_and_scan_and_scan_tests_never_see_them() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for (file, text) in [
        // The docs' weights of their own, whatever the defaults' tuning; the code's are the defaults.
        ("codeweight.toml", DOCS_ONES),
        // Each run 1: `Title`, `Some`, `words`, `here` and `#`, `.` are 6.
        ("README.md", "# Title\n\nSome words here.\n"),
        // `Hello`, `world`, `,` and `.`: 4.
        ("docs/guide.markdown", "Hello, world.\n"),
        // Under `tests/`, still docs alone: `-`, `a`, `b` are 3.
        ("tests/notes.md", "- a b\n"),
        ("node_modules/x/y.md", "# excluded\n"),
        (".hidden/h.md", "# hidden\n"),
        // The code and the tests: 3.04 each, as in the scans above.
        ("src/a.py", "def f():\n    g()\n"),
        ("tests/test_a.py", "def t():\n    g()\n"),
    ] {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    let docs = codeweight(root, &["scan", "--docs", "."]);
    assert!(docs.status.success(), "{}", stderr(&docs));
    assert_eq!(stdout(&docs), "13\n");
    let verbose = codeweight(root, &["scan", "--docs", ".", "-v"]);
    assert_eq!(
        stdout(&verbose),
        "total 13\n\
         6  README.md  (file)\n\
         4  docs/guide.markdown  (file)\n\
         3  tests/notes.md  (file)\n"
    );
    assert_eq!(
        stdout(&codeweight(root, &["scan", "--code", ".", "-v"])),
        "total 3.04\n3.04  src/a.py  f\n"
    );
    assert_eq!(
        stdout(&codeweight(root, &["scan", "--tests", ".", "-v"])),
        "total 3.04\n3.04  tests/test_a.py  t\n"
    );
    // A file given is weighed by its kind too: the code's scan of a Markdown file weighs nothing.
    assert_eq!(
        stdout(&codeweight(root, &["scan", "--code", "README.md"])),
        "0\n"
    );
    assert_eq!(
        stdout(&codeweight(root, &["scan", "--docs", "src/a.py"])),
        "0\n"
    );

    // Docs globs of one's own: only the docs folder's.
    fs::write(
        root.join("own.toml"),
        format!("{DOCS_ONES}[files]\ndocs_globs = [\"docs/**\"]\n"),
    )
    .unwrap();
    let own = codeweight(root, &["scan", "--docs", ".", "--config", "own.toml"]);
    assert!(own.status.success(), "{}", stderr(&own));
    assert_eq!(stdout(&own), "4\n");
    fs::write(root.join("bad.toml"), "[files]\ndocs_globs = [\"a/[b\"]\n").unwrap();
    let bad = codeweight(root, &["scan", "--docs", ".", "--config", "bad.toml"]);
    assert_eq!(bad.status.code(), Some(1));
    assert!(stderr(&bad).contains("docs_globs"), "{}", stderr(&bad));
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

    /// `codeweight show --code -v` (the code's whole report) under the engine's test weights (statement 1, call
    /// 2, function 100), its stdout.
    fn weigh(&self, args: &[&str]) -> String {
        self.run(&["show", "--code"], args)
    }

    /// `codeweight diff --code -v`, the uncommitted changes, as `weigh`.
    fn diff(&self, args: &[&str]) -> String {
        self.run(&["diff", "--code"], args)
    }

    /// A command (its name and its one number's flag) with `-v` and the arguments, as `weigh`.
    fn run(&self, command: &[&str], args: &[&str]) -> String {
        let weights = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/weights.toml");
        let mut all = command.to_vec();
        all.push("-v");
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
fn show_weighs_the_root_commit_against_the_empty_tree() {
    let repo = Repo::new();
    repo.commit(&[("a.py", Some(F)), ("notes.md", Some("# notes\n"))]);
    assert_eq!(
        repo.weigh(&["HEAD"]),
        "total +103\n\
         +103  added    a.py  f\n"
    );
    // No revision is `HEAD`.
    assert_eq!(repo.weigh(&[]), repo.weigh(&["HEAD"]));

    // The total delta alone, without `-v`.
    let output = codeweight(repo.path(), &["show", "--code", "HEAD"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "+3.04\n");
}

#[test]
fn show_weighs_an_add_a_change_a_delete_and_a_rename() {
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
fn show_weighs_a_merge_against_its_first_parent() {
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
fn show_weighs_a_range_tree_to_tree_so_an_undone_change_nets_zero() {
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
        serde_json::from_str(&repo.weigh(&["HEAD~2..HEAD~1", "--json"])).unwrap();
    assert_eq!(json["from"], serde_json::json!(start));
    assert_eq!(json["total"], serde_json::json!(106.0));
    assert_eq!(json["rows"].as_array().unwrap().len(), 2);
}

#[test]
fn show_leaves_out_the_excludes_and_its_tests_and_show_tests_weighs_the_tests() {
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
    // Under the defaults: f, c and u are 3.04 (function 1.55, statement 1, call 0.49), the class of the tests 5.
    assert_eq!(
        run(&["show", "--code", "HEAD"]),
        "total +6.08\n\
         +3.04  added    a.py  f\n\
         +3.04  added    c.rs  c\n"
    );
    assert_eq!(
        run(&["show", "--tests", "HEAD"]),
        "total +11.08\n\
         \x20  +5  added    c.rs  tests (class)\n\
         +3.04  added    c.rs  tests.u\n\
         +3.04  added    tests/test_a.py  f\n"
    );

    // The uncommitted changes: a test heavier by a statement and a call.
    fs::write(repo.path().join("tests/test_a.py"), FG).unwrap();
    assert_eq!(run(&["diff", "--code"]), "total 0\n");
    assert_eq!(
        run(&["diff", "--tests"]),
        "total +1.49\n+1.49  heavier  tests/test_a.py  f\n"
    );
}

#[test]
fn exclude_leaves_out_more_than_the_weights_file_s_excludes() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir(root.join(".git")).unwrap();
    for file in [
        "src/a.py",
        "src/gen/g.py",
        "gen/h.py",
        "node_modules/x/y.py",
        "b.py",
    ] {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, F).unwrap();
    }
    // The engine's test weights, with an exclude of their own.
    let weights = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/weights.toml"))
        .unwrap()
        .replace("exclude = []", "exclude = [\"**/node_modules/**\"]");
    fs::write(root.join("w.toml"), weights).unwrap();
    let scan = |args: &[&str]| {
        let mut all = vec!["scan", "--code", "--by", "file"];
        all.extend_from_slice(args);
        all.extend_from_slice(&["--config", "w.toml"]);
        let output = codeweight(root, &all);
        assert!(output.status.success(), "{}", stderr(&output));
        stdout(&output)
    };
    // The weights file's excludes (node_modules) stay; a folder at any depth, a folder from the root, a glob.
    assert_eq!(
        scan(&["--exclude", "gen/"]),
        "total 206\n\
         103  b.py\n\
         103  src/a.py\n"
    );
    assert_eq!(
        scan(&["--exclude", "src/gen/", "--exclude", "b.py"]),
        "total 206\n\
         103  gen/h.py\n\
         103  src/a.py\n"
    );
    // Matched from the repo's root, though the folder scanned is under it.
    assert_eq!(
        scan(&["src", "--exclude", "/src/gen/"]),
        "total 103\n\
         103  src/a.py\n"
    );
    // A bad one is a usage error.
    let output = codeweight(root, &["scan", "--exclude", "/"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("--exclude"), "{}", stderr(&output));
}

#[test]
fn show_and_diff_leave_out_what_exclude_matches() {
    let repo = Repo::new();
    repo.commit(&[("a.py", Some(F)), ("gen/g.py", Some(F))]);
    assert_eq!(
        repo.weigh(&["--exclude", "gen/"]),
        "total +103\n\
         +103  added    a.py  f\n"
    );
    fs::write(repo.path().join("gen/h.py"), F).unwrap();
    fs::write(repo.path().join("b.py"), F).unwrap();
    assert_eq!(
        repo.diff(&["--exclude", "gen/"]),
        "total +103\n\
         +103  added    b.py  f\n"
    );
}

#[test]
fn show_docs_weighs_a_doc_added_edited_renamed_and_deleted() {
    let repo = Repo::new();
    // Under tests/weights.toml (word 1, symbol 10, punctuation 100): `#` and 21 words are 31.
    let doc = format!("# A\n{}", "word\n".repeat(20));
    repo.commit(&[("doc.md", Some(&doc)), ("a.py", Some(F))]);
    assert_eq!(
        repo.run(&["show", "--docs"], &["HEAD"]),
        "total +31\n+31  added    doc.md  (file)\n"
    );
    // The code's delta has no docs in it.
    assert_eq!(
        repo.weigh(&["HEAD"]),
        "total +103\n+103  added    a.py  f\n"
    );

    // Two words and a `,`: 102.
    let edited = format!("{doc}more, words\n");
    repo.commit(&[("doc.md", Some(&edited))]);
    assert_eq!(
        repo.run(&["show", "--docs"], &["HEAD"]),
        "total +102\n+102  heavier  doc.md  (file)\n"
    );

    // Renamed with a word more: git sees one file, its delta the word's.
    repo.commit(&[
        ("doc.md", None),
        ("guide.md", Some(&format!("{edited}word\n"))),
    ]);
    assert_eq!(
        repo.run(&["show", "--docs"], &["HEAD"]),
        "total +1\n+1  heavier  doc.md -> guide.md  (file)\n"
    );

    repo.commit(&[("guide.md", None)]);
    assert_eq!(
        repo.run(&["show", "--docs"], &["HEAD"]),
        "total -134\n-134  removed  guide.md  (file)\n"
    );
    // A range: the first commit's doc against none.
    assert_eq!(
        repo.run(&["show", "--docs"], &["HEAD~3..HEAD"]),
        "total -31\n-31  removed  doc.md  (file)\n"
    );

    // The uncommitted changes: a doc untracked, a code file changed.
    fs::write(repo.path().join("new.md"), "x\n").unwrap();
    fs::write(repo.path().join("a.py"), FG).unwrap();
    assert_eq!(
        repo.run(&["diff", "--docs"], &[]),
        "total +1\n+1  added    new.md  (file)\n"
    );
    repo.git(&["add", "a.py"]);
    assert_eq!(repo.run(&["diff", "--docs"], &["--staged"]), "total 0\n");
}

#[test]
fn diff_weighs_the_uncommitted_changes_and_leaves_the_index_alone() {
    let repo = Repo::new();
    repo.commit(&[
        ("a.py", Some(F)),
        ("gone.py", Some("def x():\n    pass\n")),
        (".gitignore", Some("ignored.py\n")),
    ]);
    assert_eq!(repo.diff(&[]), "total 0\n");

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
        repo.diff(&[]),
        "total +109\n\
         +103  added    staged.py  f\n\
         +103  added    untracked.py  f\n\
         -100  removed  gone.py  x\n\
         \x20 +3  heavier  a.py  f\n"
    );
    assert_eq!(state(), before);
    // The staged change alone.
    assert_eq!(
        repo.diff(&["--staged"]),
        "total +103\n+103  added    staged.py  f\n"
    );
    assert_eq!(state(), before);

    let json: serde_json::Value = serde_json::from_str(&repo.diff(&["--json"])).unwrap();
    assert_eq!(
        json["from"],
        serde_json::json!(repo.git(&["rev-parse", "HEAD"]))
    );
    assert_eq!(json["to"], serde_json::Value::Null);
    assert_eq!(json["total"], serde_json::json!(109.0));
}

#[test]
fn diff_sees_a_rename_into_an_untracked_file_and_a_repo_with_no_commit() {
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
        repo.diff(&[]),
        "total +3\n+3  heavier  old.py -> new.py  h\n"
    );

    // No commit yet: against the empty tree.
    let fresh = Repo::new();
    fs::write(fresh.path().join("a.py"), F).unwrap();
    assert_eq!(fresh.diff(&[]), "total +103\n+103  added    a.py  f\n");
    // Nothing staged yet (no index at all), then the file staged.
    assert_eq!(fresh.diff(&["--staged"]), "total 0\n");
    fresh.git(&["add", "a.py"]);
    assert_eq!(
        fresh.diff(&["--staged"]),
        "total +103\n+103  added    a.py  f\n"
    );
    let json: serde_json::Value = serde_json::from_str(&fresh.diff(&["--json"])).unwrap();
    assert_eq!(json["from"], serde_json::Value::Null);
    assert_eq!(json["to"], serde_json::Value::Null);
}

// Linux only: a file's name there may be any bytes, where macOS's file systems and Windows want UTF-8.
#[cfg(target_os = "linux")]
#[test]
fn show_weighs_a_file_whose_name_isn_t_utf8() {
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
fn show_at_a_shallow_clone_s_boundary_is_an_error_not_a_root() {
    let repo = Repo::new();
    repo.commit(&[("a.py", Some(F))]);
    repo.commit(&[("a.py", Some(FG))]);
    let clone = Repo::new();
    let url = format!("file://{}", repo.path().display());
    clone.git(&["fetch", "-q", "--depth", "1", &url, "main"]);
    clone.git(&["reset", "-q", "--hard", "FETCH_HEAD"]);
    let output = codeweight(clone.path(), &["show", "--code", "HEAD"]);
    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    assert!(
        stderr(&output).contains("shallow clone"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn show_of_a_bad_revision_or_outside_a_repo_ends_with_exit_1() {
    let repo = Repo::new();
    repo.commit(&[("a.py", Some(F))]);
    let bad = codeweight(repo.path(), &["show", "nope"]);
    assert_eq!(bad.status.code(), Some(1));
    assert!(
        stderr(&bad).contains("`nope` is not a commit"),
        "{}",
        stderr(&bad)
    );
    let outside = tempfile::tempdir().unwrap();
    let none = codeweight(outside.path(), &["show", "HEAD"]);
    assert_eq!(none.status.code(), Some(1));
}

#[test]
fn show_and_diff_work_from_outside_with_c_or_repo_and_their_first_names_are_gone() {
    let repo = Repo::new();
    let weights = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/weights.toml");
    let sha = repo.commit(&[("a.py", Some(F))]);
    fs::write(repo.path().join("a.py"), FG).unwrap();
    repo.git(&["add", "a.py"]);
    let outside = tempfile::tempdir().unwrap();
    let path = repo.path().to_str().unwrap();
    let run = |args: &[&str]| {
        let mut all = args.to_vec();
        all.extend_from_slice(&["--config", weights]);
        let output = codeweight(outside.path(), &all);
        assert!(output.status.success(), "{args:?}: {}", stderr(&output));
        stdout(&output)
    };
    for repo in ["-C", "--repo"] {
        assert_eq!(
            run(&["diff", "--staged", "--short", repo, path]),
            "c+3 m0 d0 t0\n"
        );
    }
    let json: serde_json::Value =
        serde_json::from_str(&run(&["show", &sha, "--json", "-C", path])).unwrap();
    assert_eq!(json["to"], serde_json::json!(sha));
    assert_eq!(json["code"], serde_json::json!(103.0));
    // `show` and `diff` were `commit` and `pre-commit`: no alias left, an unknown command.
    for command in ["commit", "pre-commit"] {
        let output = codeweight(outside.path(), &[command, "-C", path]);
        assert_eq!(output.status.code(), Some(2), "{command}");
    }
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
            "--code",
            "A.java",
            "notes.txt",
            "--json",
            "--top",
            "1",
        ],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    // Under the defaults, as in the text: A is 5, A.f 3.04.
    assert_eq!(
        json,
        serde_json::json!({
            "total": 8.04,
            "by": "function",
            "rows": [{"path": "A.java", "kind": "class", "name": "A", "line": 1, "weight": 5.0}],
            "warnings": []
        })
    );
}

/// A folder of code, tests and docs: under the defaults, the code 3.04, the docs 6 and the tests 3.04 + 5 + 3.04;
/// the comments 3 + 4, under the docs' weights of `codeweight.toml`.
fn four_parts(root: &Path) {
    for (file, text) in [
        // f is function 1.55, statement 1, call 0.49; its comment `Calls`, `g`, `.` 3.
        ("src/a.py", "def f():\n    # Calls g.\n    g()\n"),
        // The docs' weights of their own: `Title`, `Some`, `words`, `here` and `#`, `.` are 6.
        ("codeweight.toml", DOCS_ONES),
        ("README.md", "# Title\n\nSome words here.\n"),
        // c is 3.04 of the code, its tests' class 5 and u 3.04 of the tests; c's doc comment 4 (`C`, `calls`, `g`,
        // `.`), the tests' comment nothing.
        (
            "src/c.rs",
            "/// C calls g.\nfn c() { g(); }\n#[cfg(test)]\nmod tests {\n    // Not weighed.\n    #[test]\n    \
             fn u() { c(); }\n}\n",
        ),
        // A test file's comment weighs nothing.
        ("tests/test_a.py", "def t():\n    # Not weighed.\n    g()\n"),
        // Syntax errors: weighed, with a warning.
        ("src/B.java", "class B { void f( }"),
    ] {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

#[test]
fn scan_prints_the_code_the_docs_the_tests_and_the_comments_each_as_its_flag_does() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    four_parts(root);
    let ok = |args: &[&str]| {
        let output = codeweight(root, args);
        assert!(output.status.success(), "{}", stderr(&output));
        output
    };
    let only = |flag: &str| stdout(&ok(&["scan", ".", flag])).trim().to_string();
    let (code, docs, tests) = (only("--code"), only("--docs"), only("--tests"));
    assert_eq!((docs.as_str(), tests.as_str()), ("6", "11.08"));
    assert_eq!(only("--comments"), "7");
    assert_eq!(
        stdout(&ok(&["scan", ".", "--comments", "-v"])),
        "total 7\n4  src/c.rs  c\n3  src/a.py  f\n\
         warning: src/B.java has syntax errors, weighed as tree-sitter recovered it\n"
    );

    let three = ok(&["scan", "."]);
    assert_eq!(
        stdout(&three),
        format!("code +{code}\ncomments +7\ndocs +{docs}\ntests +{tests}\n")
    );
    assert_eq!(stderr(&three), "");
    // `-v`: the warnings to stderr, the numbers the same.
    let verbose = ok(&["scan", ".", "-v"]);
    assert_eq!(stdout(&verbose), stdout(&three));
    assert_eq!(
        stderr(&verbose),
        "warning: src/B.java has syntax errors, weighed as tree-sitter recovered it\n"
    );

    let short = ok(&["scan", ".", "--short"]);
    let code: f64 = code.parse().unwrap();
    assert_eq!(stdout(&short), format!("c+{} m+7 d+6 t+11\n", code.round()));
    assert_eq!(stderr(&short), "");
    let short = ok(&["scan", ".", "--short", "-v"]);
    assert!(stderr(&short).starts_with("warning: src/B.java has syntax errors"));

    let json: serde_json::Value =
        serde_json::from_str(&stdout(&ok(&["scan", ".", "--json"]))).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "code": code,
            "docs": 6.0,
            "tests": 11.08,
            "comments": 7.0,
            "warnings": [{"path": "src/B.java", "kind": "syntax_errors", "side": null}]
        })
    );

    // Nothing of a kind: a zero has no sign.
    assert_eq!(
        stdout(&ok(&["scan", "README.md"])),
        "code 0\ncomments 0\ndocs +6\ntests 0\n"
    );
    assert_eq!(
        stdout(&ok(&["scan", "README.md", "--short"])),
        "c0 m0 d+6 t0\n"
    );
}

#[test]
fn show_and_diff_print_the_four_deltas() {
    let repo = Repo::new();
    // The test weights, `tests/` the test files.
    let weights_text =
        fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/weights.toml"))
            .unwrap()
            .replace("test_globs = []", "test_globs = [\"tests/**\"]");
    let own = tempfile::tempdir().unwrap();
    let weights = own.path().join("weights.toml");
    fs::write(&weights, weights_text).unwrap();
    let weights = weights.to_str().unwrap();
    let run = |args: &[&str]| {
        let mut all = args.to_vec();
        all.extend_from_slice(&["--config", weights]);
        let output = codeweight(repo.path(), &all);
        assert!(output.status.success(), "{}", stderr(&output));
        stdout(&output)
    };
    // Under tests/weights.toml: F's f is 103, the test's too, FG's 106; `# A` and a word are 10 + 2.
    let first = repo.commit(&[
        ("a.py", Some(F)),
        ("tests/test_a.py", Some(FG)),
        ("doc.md", Some("# A word\n")),
    ]);
    assert_eq!(
        run(&["show", "HEAD"]),
        "code +103\ncomments 0\ndocs +12\ntests +106\n"
    );
    // A test made lighter, a doc deleted, the code untouched.
    let second = repo.commit(&[("tests/test_a.py", Some(F)), ("doc.md", None)]);
    assert_eq!(
        run(&["show", "HEAD"]),
        "code 0\ncomments 0\ndocs -12\ntests -3\n"
    );
    assert_eq!(run(&["show", "HEAD", "--short"]), "c0 m0 d-12 t-3\n");
    let json: serde_json::Value =
        serde_json::from_str(&run(&["show", "HEAD~1..HEAD", "--json"])).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "from": first, "to": second, "code": 0.0, "docs": -12.0, "tests": -3.0, "comments": 0.0,
            "warnings": []
        })
    );
    // A commit that only adds comments: `Calls g` 2 to the code's, the test's nothing.
    let third = repo.commit(&[
        ("a.py", Some("def f():\n    # Calls g\n    g()\n")),
        (
            "tests/test_a.py",
            Some("def f():\n    # Not weighed\n    g()\n"),
        ),
    ]);
    assert_eq!(
        run(&["show", "HEAD"]),
        "code 0\ncomments +2\ndocs 0\ntests 0\n"
    );
    assert_eq!(run(&["show", "HEAD", "--short"]), "c0 m+2 d0 t0\n");
    assert_eq!(
        run(&["show", "HEAD", "--comments", "-v"]),
        "total +2\n+2  heavier  a.py  f\n"
    );

    // The uncommitted changes: the code heavier, a doc untracked, the staged one alone.
    fs::write(repo.path().join("a.py"), FG).unwrap();
    fs::write(repo.path().join("new.md"), "x\n").unwrap();
    assert_eq!(run(&["diff", "--short"]), "c+3 m-2 d+1 t0\n");
    repo.git(&["add", "a.py"]);
    assert_eq!(
        run(&["diff", "--staged"]),
        "code +3\ncomments -2\ndocs 0\ntests 0\n"
    );
    let json: serde_json::Value = serde_json::from_str(&run(&["diff", "--json"])).unwrap();
    assert_eq!(json["from"], serde_json::json!(third));
    assert_eq!(json["comments"], serde_json::json!(-2.0));
    assert_eq!(json["to"], serde_json::Value::Null);
    assert_eq!(json["code"], serde_json::json!(3.0));
}

#[test]
fn the_report_s_options_need_one_number_and_short_none() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("A.java"), JAVA).unwrap();
    for args in [
        &["scan", "A.java", "--top", "1"][..],
        &["scan", "A.java", "--by", "file"],
        &["scan", "A.java", "--code", "--tests"],
        &["scan", "A.java", "--docs", "--short"],
        &["scan", "A.java", "--comments", "--short"],
        &["scan", "A.java", "--comments", "--docs"],
        &["scan", "A.java", "--short", "--json"],
        &["diff", "--by", "function"],
        &["diff", "--tests", "--short"],
        &["scan", "A.java", "--format", "json"],
        &["diff", "--only-staged"],
        &["init", "--code"],
    ] {
        let output = codeweight(dir.path(), args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            stdout(&output)
        );
        assert!(
            stderr(&output).starts_with("error: "),
            "{args:?}: {}",
            stderr(&output)
        );
    }
    // `--config` stays every command's, before the command too.
    fs::write(dir.path().join("w.toml"), "[weights]\nclass = 100\n").unwrap();
    let output = codeweight(
        dir.path(),
        &["--config", "w.toml", "scan", "--code", "A.java"],
    );
    assert_eq!(stdout(&output), "103.04\n");
}

#[test]
fn every_command_s_help_lists_the_options_in_the_same_order_config_last() {
    let dir = tempfile::tempdir().unwrap();
    let report = [
        "--code",
        "--tests",
        "--docs",
        "--comments",
        "--json",
        "--short",
        "--verbose",
        "--top",
        "--by",
    ];
    for (command, own) in [
        ("scan", &["--exclude"][..]),
        ("show", &["-C", "--exclude"][..]),
        ("diff", &["-C", "--staged", "--exclude"][..]),
        ("init", &[][..]),
    ] {
        let output = codeweight(dir.path(), &[command, "-h"]);
        assert!(output.status.success(), "{}", stderr(&output));
        let help = stdout(&output);
        let options: Vec<&str> = help
            .lines()
            .skip_while(|line| *line != "Options:")
            .skip(1)
            .filter_map(|line| line.split_whitespace().find(|word| word.starts_with('-')))
            .map(|word| word.trim_end_matches(','))
            .map(|word| match word {
                "-v" => "--verbose",
                "-h" => "--help",
                word => word,
            })
            .collect();
        let mut expected = own.to_vec();
        if command != "init" {
            expected.extend(report);
        }
        expected.extend(["--config", "--help"]);
        assert_eq!(options, expected, "{command}");
    }
}
