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
    // Under the defaults: A.f is function 3, statement 1 and call 1; A is class 5.
    let output = codeweight(dir.path(), &["scan", "A.java"]);
    assert!(output.status.success(), "{}", stderr(&output));
    // Equal weights: by name.
    assert_eq!(
        stdout(&output),
        "total 10\n5  A.java  A (class)\n5  A.java  A.f\n"
    );

    let by_file = codeweight(dir.path(), &["scan", "A.java", "--by", "file"]);
    assert_eq!(stdout(&by_file), "total 10\n10  A.java\n");
    let top = codeweight(dir.path(), &["scan", "A.java", "--top", "1"]);
    assert_eq!(stdout(&top), "total 10\n5  A.java  A (class)\n");
}

#[test]
fn scan_skips_and_lists_an_unsupported_file_and_warns_of_syntax_errors() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("notes.txt"), "hello").unwrap();
    fs::write(dir.path().join("B.java"), "class B { void f( }").unwrap();
    let output = codeweight(dir.path(), &["scan", "notes.txt", "B.java"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("skipped: notes.txt"), "{out}");
    assert!(out.contains("warning: B.java has syntax errors"), "{out}");
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

    assert_eq!(total(&elsewhere, &[]), "total 10");
    fs::write(repo.join("codeweight.toml"), "[weights]\nclass = 100\n").unwrap();
    assert_eq!(total(&elsewhere, &[]), "total 105");
    fs::write(
        elsewhere.join("codeweight.toml"),
        "[weights]\nclass = 1000\n",
    )
    .unwrap();
    assert_eq!(total(&elsewhere, &[]), "total 1005");
    fs::write(elsewhere.join("given.toml"), "[weights]\nfunction = 0\n").unwrap();
    assert_eq!(total(&elsewhere, &["--config", "given.toml"]), "total 7");
}

#[test]
fn scan_walks_a_folder_its_excludes_left_out_its_tests_times_the_test_factor() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for (file, text) in [
        // Under the defaults: f is function 3, statement 1, call 1.
        ("src/a.py", "def f():\n    g()\n"),
        // h is function 3 and its call 1; the file's declaration statement 1.
        ("src/b.ts", "export const h = () => g();\n"),
        // t is 5, times the test factor 0.5.
        ("tests/test_a.py", "def t():\n    g()\n"),
        ("node_modules/x/y.ts", "g();\n"),
        ("src/notes.md", "# notes\n"),
    ] {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fs::write(root.join("codeweight.toml"), "[files]\ntest_factor = 0.5\n").unwrap();

    let output = codeweight(root, &["scan", "."]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "total 12.5\n\
         \x20 5  src/a.py  f\n\
         \x20 4  src/b.ts  h\n\
         2.5  tests/test_a.py  t\n\
         \x20 1  src/b.ts  (file)\n\
         skipped: codeweight.toml (not a language codeweight reads)\n\
         skipped: src/notes.md (not a language codeweight reads)\n"
    );

    let by_file = codeweight(root, &["scan", "src", "tests", "--by", "file"]);
    assert_eq!(
        stdout(&by_file),
        "total 12.5\n  5  src/a.py\n  5  src/b.ts\n2.5  tests/test_a.py\n\
         skipped: src/notes.md (not a language codeweight reads)\n"
    );

    fs::write(root.join("codeweight.toml"), "[files]\ntest_factor = 0\n").unwrap();
    let no_tests = codeweight(root, &["scan", "src", "tests", "--by", "file"]);
    assert_eq!(
        stdout(&no_tests).lines().next(),
        Some("total 10"),
        "{}",
        stdout(&no_tests)
    );
}
