use heng_judger::lang::c_cpp::{CCpp, CCppStd};
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

fn test_aot_lang(
    workspace_name: &str,
    lang: &dyn Language,
    source_code: &str,
    expected_output: &str,
) -> Result<()> {
    assert!(lang.needs_compile());

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

    let limit = lang::Limit {
        cpu_time: 1000,
        memory: 256 * 1024 * 1024, // 256 MiB
        output: 256 * 1024 * 1024, // 256 MiB
        pids: 16,
    };

    let compile_output = lang
        .compile(workspace.clone(), &limit)
        .context("failed to compile code")?;

    debug!(?compile_output);
    if !compile_output.is_success() {
        let msg_path = workspace.join(lang.msg_name());
        let ce_msg = fs::read_to_string(&msg_path)
            .with_context(|| format!("failed to read CE message: path = {}", msg_path.display()))?;
        error!("compile error:\n{}", ce_msg);
    }

    assert!(compile_output.is_success());

    let userout_path = workspace.join("__user_out");
    let sandbox_output = lang
        .run(
            workspace,
            "/dev/null".into(),
            userout_path.clone(),
            "/dev/null".into(),
            &limit,
        )
        .context("failed to run user process")?;

    debug!(?sandbox_output);
    assert!(sandbox_output.is_success());
    let userout = fs::read_to_string(&userout_path)?;
    assert_eq!(userout, expected_output);

    Ok(())
}

#[test]
fn lang_c_cpp() -> Result<()> {
    let ccpp = CCpp {
        std: CCppStd::Cpp11,
        o2: true,
    };

    let source_code =
        "#include<iostream>\nint main(){ std::cout<<\"hello\"<<std::endl; return 0; }\n";
    let expected_output = "hello\n";

    test_aot_lang("__test_c_cpp", &ccpp, source_code, expected_output)?;

    Ok(())
}

#[test]
fn lang_rust() -> Result<()> {
    let rust = Rust { o2: true };

    let source_code = "fn main(){ println!(\"hello\"); }";
    let expected_output = "hello\n";

    test_aot_lang("__test_rust", &rust, source_code, expected_output)?;

    Ok(())
}
