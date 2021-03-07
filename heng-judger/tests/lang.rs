use heng_judger::lang::c_cpp::{CCpp, CCppStd};
use heng_judger::lang::java::Java;
use heng_judger::lang::javascript::JavaScript;
use heng_judger::lang::python::Python;
use heng_judger::lang::rust::Rust;
use heng_judger::lang::Language;
use heng_judger::{lang, Config};

use heng_utils::container::{inject, Container};
use heng_utils::tracing::setup_tracing;

use std::fs;
use std::sync::{Arc, Once};

use anyhow::{Context, Result};
use tracing::{debug, error};

fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        std::env::set_var("RUST_LOG", "debug");
        setup_tracing();
        let config = Config::from_file("heng-judger.toml").unwrap();
        let mut container = Container::new();
        container.register(Arc::new(config));
        container.install_global();
    });
}

fn test_lang(
    workspace_name: &str,
    lang: &dyn Language,
    source_code: &str,
    expected_output: &str,
) -> Result<()> {
    init();

    let config = inject::<Config>();
    let workspace_root = &config.executor.workspace_root;

    let workspace = workspace_root.join(workspace_name);
    if workspace.exists() {
        let _ = fs::remove_dir_all(&workspace);
    }
    fs::create_dir_all(&workspace)?;

    let src_path = workspace.join(lang.src_name());

    fs::write(&src_path, source_code)?;

    let compile_limit = lang::Limit {
        cpu_time: 3000,
        memory: config.executor.hard_limit.memory.as_u64(),
        output: config.executor.hard_limit.output.as_u64(),
        pids: config.executor.hard_limit.pids,
    };

    if lang.needs_compile() {
        let compile_output = lang
            .compile(workspace.clone(), &compile_limit)
            .context("failed to compile code")?;

        debug!(name=?lang.lang_name(), ?compile_output);
        if !compile_output.is_success() {
            let msg_path = workspace.join(lang.msg_name());
            let ce_msg = fs::read_to_string(&msg_path).with_context(|| {
                format!("failed to read CE message: path = {}", msg_path.display())
            })?;
            error!("compile error:\n{}", ce_msg);
        }

        assert!(compile_output.is_success());
    }

    let runtime_limit = lang::Limit {
        cpu_time: 1000,
        ..compile_limit
    };

    let userout_path = workspace.join("__user_out");
    let sandbox_output = lang
        .run(
            workspace,
            "/dev/null".into(),
            userout_path.clone(),
            "/dev/null".into(),
            &runtime_limit,
        )
        .context("failed to run user process")?;

    debug!(name=?lang.lang_name(), ?sandbox_output);
    assert!(sandbox_output.is_success());
    let userout = fs::read_to_string(&userout_path)?;
    assert_eq!(userout, expected_output);

    Ok(())
}

#[test]
fn lang_cpp() -> Result<()> {
    let cpp = CCpp {
        std: CCppStd::Cpp11,
        o2: true,
    };

    let source_code = r#"
        #include<iostream>
        int main() { 
            std::cout<<"hello"<<std::endl;
            return 0;
        }
    "#;
    let expected_output = "hello\n";

    test_lang("__test_cpp", &cpp, source_code, expected_output)
}

#[test]
fn lang_c() -> Result<()> {
    let c = CCpp {
        std: CCppStd::C11,
        o2: true,
    };

    let source_code = r#"
        #include<stdio.h>
        int main() { 
            printf("hello\n");
            return 0;
        }
    "#;
    let expected_output = "hello\n";

    test_lang("__test_c", &c, source_code, expected_output)
}

#[test]
fn lang_rust() -> Result<()> {
    let rust = Rust { o2: true };

    let source_code = r#"
        fn main() {
            println!("hello");
        }
    "#;
    let expected_output = "hello\n";

    test_lang("__test_rust", &rust, source_code, expected_output)
}

#[test]
fn lang_java() -> Result<()> {
    let java = Java {};

    let source_code = r#"
        public class Main {
            public static void main(String[] args) {
                System.out.println("hello");
            }
        }
    "#;
    let expected_output = "hello\n";

    test_lang("__test_java", &java, source_code, expected_output)
}

#[test]
fn lang_python() -> Result<()> {
    let python = Python {};

    let source_code = r#"print("hello")"#;
    let expected_output = "hello\n";

    test_lang("__test_python", &python, source_code, expected_output)
}

#[test]
fn lang_javascript() -> Result<()> {
    let js = JavaScript {};

    let source_code = r#"console.log("hello")"#;
    let expected_output = "hello\n";

    test_lang("__test_javascript", &js, source_code, expected_output)
}
