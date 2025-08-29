use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

struct RockTestFile {
    name: String,
    prelude: String,
    tests: Vec<RockTest>,
}

#[derive(Debug)]
struct RockTest {
    check: bool,    //use `check` instead of `run`
    entry: String,  //test name
    expect: String, //expected output
    source: String, //test source code
}

struct RockTestEnv {
    main_path: PathBuf,
    test_path: PathBuf,
}

fn main() {
    let test_src = if fs::exists("./rock_test/src").unwrap() {
        PathBuf::from("./rock_test/src")
    } else {
        PathBuf::from("./src")
    };

    let test_files = parse_tests();
    let test_env = setup_test_env(test_src);
    run_tests(test_env, test_files);
}

fn parse_tests() -> Vec<RockTestFile> {
    let test_src_path =
        if fs::exists("./rock_test/src").unwrap() { "./rock_test/src" } else { "./src" };

    let read_dir = fs::read_dir(test_src_path).expect("`./rock_test/src` or `./src` must exist");
    let mut test_files = vec![];
    let mut test_set = HashSet::with_capacity(64);

    for entry in read_dir {
        let dir = entry.expect("valid dir entry");
        let path = dir.path();
        let text = fs::read_to_string(&path).expect("read_to_string");
        let extension = path.extension().unwrap().to_str().unwrap();

        if extension != "rock" {
            continue;
        }
        let test = parse_test_file(&path, &mut test_set, text);
        test_files.push(test);
    }

    test_files
}

fn parse_test_file(path: &Path, test_set: &mut HashSet<String>, text: String) -> RockTestFile {
    let mut tests = Vec::with_capacity(32);
    let mut lines = text.lines().peekable();

    let prelude = if lines.next_if(|&l| l.starts_with("//#prelude")).is_some() {
        parse_source_code(&mut lines)
    } else {
        "".into()
    };

    while lines.peek().is_some() {
        let (check, entry) = parse_entry_name(&mut lines);
        let expect = parse_expect(&mut lines);
        let source = parse_source_code(&mut lines);
        let test = RockTest { check, entry, expect, source };

        if test_set.contains(&test.entry) {
            panic!("duplicate test found: `{}` in `{}`", test.entry, path.to_string_lossy(),)
        }
        test_set.insert(test.entry.clone());
        tests.push(test);
    }

    let name = path.file_name().unwrap().to_str().unwrap().to_string();
    RockTestFile { name, prelude, tests }
}

fn parse_entry_name(lines: &mut std::iter::Peekable<std::str::Lines>) -> (bool, String) {
    let entry_line = lines.next().expect("expected `#entry \"<test_name>\"`");
    let start = entry_line.find('"').expect("expected opening \"") + 1;
    let end = entry_line[start..].find('"').expect("expected closing \"") + start;
    let entry = entry_line[start..end].to_string();
    (entry_line.starts_with("//#entry(check)"), entry)
}

fn parse_expect(lines: &mut std::iter::Peekable<std::str::Lines>) -> String {
    let Some(expect_line) = lines.next_if(|&l| l.starts_with("//#expect")) else {
        return "".into();
    };
    let mut expect = String::with_capacity(128);
    while let Some(line) = lines.peek() {
        if line.starts_with("//#!") {
            lines.next();
            break;
        }
        assert!(line.starts_with("//"));
        expect.push_str(&line[2..]);
        expect.push('\n');
        lines.next();
    }
    if expect_line == "//#expect(no_endl)" {
        expect.pop();
    }
    expect
}

fn parse_source_code(lines: &mut std::iter::Peekable<std::str::Lines>) -> String {
    let mut source = String::with_capacity(256);
    while let Some(line) = lines.peek() {
        if line.starts_with("//#") {
            break;
        }
        source.push_str(line);
        source.push('\n');
        lines.next();
    }
    source
}

fn setup_test_env(test_src: PathBuf) -> RockTestEnv {
    let mut test_root = test_src.canonicalize().unwrap();
    test_root.pop();
    let run_root = test_root.join("run");
    let src_root = run_root.join("src");
    let main_path = src_root.join("main.rock");
    let test_path = src_root.join("test.rock");
    let manifest_path = run_root.join("Rock.toml");

    if !fs::exists(&run_root).unwrap() {
        fs::create_dir(&run_root).unwrap();
    }
    if !fs::exists(&src_root).unwrap() {
        fs::create_dir(&src_root).unwrap();
    }

    let manifest = r#"
[package]
name = "run"
kind = "bin"
version = "0.1.0"
[build]
[dependencies]"#;

    fs::write(&manifest_path, manifest).unwrap();
    std::env::set_current_dir(&run_root).unwrap();

    RockTestEnv { main_path, test_path }
}

fn run_tests(test_env: RockTestEnv, test_files: Vec<RockTestFile>) {
    use std::process::Command;

    const R: &str = "\x1B[0m";
    const RB: &str = "\x1B[1;31m";
    const GB: &str = "\x1B[1;32m";
    const CB: &str = "\x1B[1;36m";

    let mut total_count = 0;
    let mut passed_count = 0;
    let mut trailing_endl = true;

    let timer = std::time::Instant::now();
    for test_file in test_files {
        println!("\n{CB}src/{}{R}", test_file.name);

        for test in test_file.tests {
            let main_src = if test.check {
                "proc main() void {}".to_string()
            } else {
                format!("import test.{{test_{0}}}\nproc main() void {{ test_{0}(); }}", test.entry)
            };
            let test_src = format!("{}{}", test.source, test_file.prelude);

            fs::write(&test_env.main_path, main_src).unwrap();
            fs::write(&test_env.test_path, test_src).unwrap();

            let output = Command::new("rock").arg("r").output().unwrap();
            let output_out = String::from_utf8_lossy(&output.stdout).into_owned();
            let output_err = String::from_utf8_lossy(&output.stderr).into_owned();

            //prevent stderr & stdout overlap
            if !output_out.is_empty() && !output_err.is_empty() {
                eprintln!("expected test outputs to be either stderr or stdout, not combined:");
                eprintln!("[stdout]:\n{output_out}");
                eprintln!("[stderr]:\n{output_err}");
                panic!("cannot verify outputs");
            }

            //trim feedback from stdout
            let output = if output_out.starts_with("  Finished") {
                output_out.lines().skip(3).collect::<Vec<&str>>().join("\n")
            } else if !output_out.is_empty() {
                output_out
            } else {
                output_err
            };

            total_count += 1;
            if output == test.expect {
                passed_count += 1;
                trailing_endl = true;
                println!("{:.<48} [{GB}OK{R}]", test.entry);
            } else {
                trailing_endl = false;
                println!("{:.<48} [{RB}ERROR{R}]", test.entry);
                println!("\n{RB}expected output:{R}\n{}{CB}[end]{R}", test.expect);
                println!("{RB}received output:{R}\n{}{CB}[end]{R}\n", output);
            }
        }
    }
    let runtime = timer.elapsed().as_secs_f64();

    if trailing_endl {
        println!();
    }
    let color = if passed_count == total_count { GB } else { RB };
    println!("{color}test runtime:{R} {runtime:.2} sec");
    println!("{color}tests passed:{R} [{passed_count}/{total_count}]\n");
}
