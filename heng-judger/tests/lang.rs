use heng_judger::lang::cpp::{Cpp, CppStd};
use heng_judger::lang::Language;
use heng_judger::{lang, Config};

use heng_utils::container::{inject, Container};
use heng_utils::tracing::setup_tracing;

use std::fs;
use std::sync::{Arc, Once};

use anyhow::{Context, Result};
use tracing::debug;

fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        dotenv::dotenv().ok();
        setup_tracing();
        let config = Config::from_file("heng-judger.toml").unwrap();
        let mut container = Container::new();
        container.register(Arc::new(config));
        container.install_global();
    });
}

#[test]
fn c_cpp() -> Result<()> {
    init();

    let config = inject::<Config>();
    let workspace_path = &config.executor.workspace_root;

    let workspace = workspace_path.join("__test_c_cpp");
    if workspace.exists() {
        let _ = fs::remove_dir_all(&workspace);
    }
    fs::create_dir_all(&workspace)?;

    let src_path = workspace.join("__user_code");
    fs::write(
        &src_path,
        "#include<iostream>\nint main(){ std::cout<<\"hello\"<<std::endl; return 0; }\n",
    )?;

    let cpp = Cpp {
        std: CppStd::Cpp11,
        o2: true,
    };

    let limit = lang::Limit {
        cpu_time: 1000,
        memory: 256 * 1024 * 1024, // 256 MiB
        output: 256 * 1024 * 1024, // 256 MiB
        pids: 16,
    };

    let compile_output = cpp
        .compile(&src_path, limit)
        .context("failed to compile c/cpp code")?;

    debug!(?compile_output);
    if !compile_output.is_success() {
        let ce_msg = fs::read_to_string(&compile_output.ce_path)?;
        debug!(?ce_msg);
    }

    assert!(compile_output.is_success());

    Ok(())
}
