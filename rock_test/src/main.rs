use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

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

#[derive(Debug)]
struct RockTestFile {
    name: String,
    prelude: String,
    tests: HashMap<String, RockTest>,
}

#[derive(Debug)]
struct RockTest {
    entry: String,
    expect: String,
    source: String,
}

struct RockTestEnv {
    main_path: PathBuf,
    test_path: PathBuf,
}

fn parse_tests() -> Vec<RockTestFile> {
    let test_src_path = if fs::exists("./rock_test/src").unwrap() {
        "./rock_test/src"
    } else {
        "./src"
    };

    let read_dir = fs::read_dir(test_src_path).expect("`./rock_test/src` or `./src` must exist");
    let mut test_files = vec![];

    for entry in read_dir {
        let dir = entry.expect("valid dir entry");
        let path = dir.path();
        let text = fs::read_to_string(&path).expect("read_to_string");
        let extension = path.extension().unwrap().to_str().unwrap();

        if extension != "rock" {
            continue;
        }
        let test = parse_test_file(&path, text);
        test_files.push(test);
    }

    test_files
}

fn parse_test_file(path: &PathBuf, text: String) -> RockTestFile {
    let mut lines = text.lines().peekable();
    let mut prelude = String::with_capacity(256);

    let prelude_line = lines.next().expect("#prelude line");
    assert_eq!(
        prelude_line,
        "//#prelude",
        "test file `{}` expected `//#prelude` tag",
        path.to_string_lossy()
    );

    while let Some(line) = lines.peek() {
        if line.starts_with("//#") {
            break;
        }
        prelude.push_str(line);
        prelude.push('\n');
        lines.next();
    }
    prelude.pop();

    let mut tests = HashMap::with_capacity(16);

    while lines.peek().is_some() {
        let entry_line = lines.next().expect("#entry line");
        assert!(
            entry_line.starts_with("//#entry"),
            "test file `{}` expected `//#entry` tag",
            path.to_string_lossy()
        );
        let start_idx = entry_line
            .find('"')
            .expect("expected entry name opening \"")
            + 1;
        let end_idx = entry_line[start_idx..]
            .find('"')
            .expect("expected entry name closing \"")
            + start_idx;
        let entry = entry_line[start_idx..end_idx].to_string();

        let expect_line = lines.next().expect("#expect line");
        let expect = if expect_line.starts_with("//#expect") {
            let mut expect = String::with_capacity(128);
            while let Some(line) = lines.peek() {
                assert!(line.starts_with("//"));
                if line.starts_with("//#!") {
                    lines.next();
                    break;
                }
                expect.push_str(&line[2..]);
                expect.push('\n');
                lines.next();
            }
            if expect_line == "//#expect(no_new_line)" {
                expect.pop();
            }
            expect
        } else {
            panic!(
                "test file `{}` expected `//#expect` tag",
                path.to_string_lossy()
            )
        };

        let mut source = String::with_capacity(128);
        while let Some(line) = lines.peek() {
            if line.starts_with("//#") {
                break;
            }
            source.push_str(line);
            source.push('\n');
            lines.next();
        }

        let test = RockTest {
            entry,
            expect,
            source,
        };

        let existing = tests.insert(test.entry.clone(), test);
        if let Some(duplicate) = existing {
            panic!(
                "test file `{}` has multiple tests named `{}`",
                path.to_string_lossy(),
                duplicate.entry
            )
        }
    }

    RockTestFile {
        name: path.file_name().unwrap().to_str().unwrap().to_string(),
        prelude,
        tests,
    }
}

fn setup_test_env(test_src: PathBuf) -> RockTestEnv {
    let mut abs_path = test_src.canonicalize().unwrap();
    abs_path.pop();

    let run_root = abs_path.join("run");
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

    fs::write(&main_path, "").unwrap();
    fs::write(&test_path, "").unwrap();
    fs::write(&manifest_path, manifest).unwrap();
    std::env::set_current_dir(&run_root).unwrap();

    RockTestEnv {
        main_path,
        test_path,
    }
}

fn run_tests(test_env: RockTestEnv, test_files: Vec<RockTestFile>) {
    use std::process::Command;

    const R: &str = "\x1B[0m";
    const RB: &str = "\x1B[1;31m";
    const GB: &str = "\x1B[1;32m";
    const CB: &str = "\x1B[1;36m";

    let mut total_count = 0;
    let mut passed_count = 0;

    for test_file in test_files {
        println!("\n{CB}src/{}{R}", test_file.name);

        for (_, test) in test_file.tests {
            let main_src = format!(
                "import test.{{test_{}}}\nproc main() s32 {{ test_{}(); return 0; }}",
                test.entry, test.entry
            );
            let test_src = format!("{}{}", test.source, test_file.prelude);
            fs::write(&test_env.main_path, main_src).unwrap();
            fs::write(&test_env.test_path, test_src).unwrap();

            //@run in release to skip debug info (faster)
            let output = Command::new("rock")
                .arg("r")
                .arg("--release")
                .output()
                .unwrap();

            //prevent stderr & stdout overlap
            let output_out = String::from_utf8_lossy(&output.stdout).into_owned();
            let output_err = String::from_utf8_lossy(&output.stderr).into_owned();
            if !output_out.is_empty() && !output_err.is_empty() {
                panic!("expected test outputs to be either stderr or stdout, not combined");
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
                println!("{:.<40} [{GB}OK{R}]", test.entry);
            } else {
                println!("{:.<40} [{RB}ERROR{R}]", test.entry);
                println!("\n{RB}expected output:{R}\n{}{CB}[end]{R}", test.expect);
                println!("{RB}received output:{R}\n{}{CB}[end]{R}\n", output);
            }
        }
    }

    let result_color = if passed_count == total_count { GB } else { RB };
    println!(
        "\n{result_color}tests passed:{R} [{}/{}]\n",
        passed_count, total_count
    );
}
