mod common;

use anyhow::Result;
use heng_sandbox::{SandboxArgs, SandboxOutput};
use log::info;

fn gcc_compile(src: &str, bin: &str) -> Result<SandboxOutput> {
    let args = SandboxArgs {
        bin: "gcc".to_owned(),
        args: vec!["-o".to_owned(), bin.to_owned(), src.to_owned()],
        memory_limit: Some(256 * 1024), // 256 MiB
        real_time_limit: Some(3000),    // 3000 ms
        ..Default::default()
    };

    common::run(&args)
}

fn test_compile(name: &str, src: &str, bin: &str, check: impl FnOnce(SandboxOutput)) -> Result<()> {
    common::init();
    info!("{} src = {}, bin = {}", name, src, bin);
    let output = gcc_compile(src, bin)?;
    check(output);
    info!("{} finished", name);
    Ok(())
}

fn test_hack(
    name: &str,
    src: &str,
    bin: &str,
    args: &SandboxArgs,
    check: impl FnOnce(SandboxOutput),
) -> Result<()> {
    common::init();
    info!("{} src = {}, bin = {}", name, src, bin);
    gcc_compile(src, bin)?;
    info!("{} run hack", name);
    let output = common::run(&args)?;
    check(output);
    info!("{} finished", name);
    Ok(())
}

macro_rules! assets {
    ($file:literal) => {
        concat!("tests/assets/", $file)
    };
}

macro_rules! tmp {
    ($file:literal) => {
        concat!("/tmp/", $file)
    };
}

macro_rules! assert_le {
    ($lhs:expr, $rhs:expr) => {{
        let lhs = $lhs;
        let rhs = $rhs;
        assert!(lhs <= rhs, "lhs = {:?}, rhs = {:?}", lhs, rhs)
    }};
}

#[tokio::test(flavor = "multi_thread")]
async fn t01_empty() -> Result<()> {
    let name = "t01_empty";
    let src = assets!("empty.c");
    let bin = tmp!("t01_empty");

    let args = &SandboxArgs {
        bin: bin.to_owned(),
        ..Default::default()
    };

    test_hack(name, src, bin, args, |output| {
        assert_eq!(output.code, 0);
        assert_eq!(output.signal, 0);
        assert_eq!(output.status, 0);

        assert_le!(output.real_time, 10);
        assert_eq!(output.sys_time, 0);
        assert_le!(output.user_time, 1);
        assert_le!(output.cpu_time, 1);
        assert_le!(output.memory, 400);
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn t02_sleep() -> Result<()> {
    let name = "t02_sleep";
    let src = assets!("sleep.c");
    let bin = tmp!("t02_sleep");

    let args = &SandboxArgs {
        bin: bin.to_owned(),
        real_time_limit: Some(1000),
        ..Default::default()
    };

    test_hack(name, src, bin, args, |output| {
        assert_eq!(output.code, 0);
        assert_eq!(output.signal, 9);
        assert_eq!(output.status, 9);

        assert_le!(output.real_time, 1010);
        assert_eq!(output.sys_time, 0);
        assert_le!(output.user_time, 1);
        assert_le!(output.cpu_time, 1);
        assert_le!(output.memory, 400);
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn t03_forkbomb() -> Result<()> {
    let name = "t03_forkbomb";
    let src = assets!("forkbomb.c");
    let bin = tmp!("t03_forkbomb");

    let args = &SandboxArgs {
        bin: bin.to_owned(),
        max_pids_limit: Some(3),
        real_time_limit: Some(1000),
        stdout: Some(tmp!("t03_forkbomb_stdout").into()),
        ..Default::default()
    };

    test_hack(name, src, bin, args, |output| {
        assert_eq!(output.code, 0);
        assert_eq!(output.signal, 9);
        assert_eq!(output.status, 9);

        assert_le!(output.real_time, 1010);
        assert_eq!(output.sys_time, 0);
        assert_le!(output.user_time, 3000);
        assert_le!(output.cpu_time, 3000);
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn t04_includebomb() -> Result<()> {
    let name = "t04_includebomb";
    let src = assets!("includebomb.c");
    let bin = tmp!("t04_includebomb");

    test_compile(name, src, bin, |output| {
        if output.code == 0 {
            assert!(output.memory >= 256 * 1024);
        }
        assert_le!(output.real_time, 3010);
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn t05_oom() -> Result<()> {
    let name = "t05_oom";
    let src = assets!("oom.c");
    let bin = tmp!("t05_oom");

    let args = &SandboxArgs {
        bin: bin.to_owned(),
        memory_limit: Some(16 * 1024), // 16 MiB
        real_time_limit: Some(1000),
        ..Default::default()
    };

    test_hack(name, src, bin, args, |output| {
        assert_eq!(output.code, 0);
        assert_eq!(output.signal, 9);
        assert_eq!(output.status, 9);

        assert_le!(output.real_time, 1010);
        assert_eq!(output.sys_time, 0);
        assert_le!(output.user_time, 1000);
        assert_le!(output.cpu_time, 1000);
    })
}