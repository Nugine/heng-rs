// use std::sync::atomic::{AtomicI32, Ordering::Relaxed};
use std::time::Duration;

use log::debug;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use tokio::task::{self, JoinHandle};
use tokio::time;

// static CHILD_PID: AtomicI32 = AtomicI32::new(i32::min_value());

// extern "C" fn on_sigalrm(_arg: i32) {
//     let child_pid = CHILD_PID.load(Relaxed);
//     if child_pid != i32::min_value() {
//         let _ = signal::kill(Pid::from_raw(child_pid), Signal::SIGKILL);
//     }
// }

// pub fn alarm_kill(child_pid: Pid, timeout_secs: u32) -> nix::Result<()> {
//     CHILD_PID.store(child_pid.as_raw(), Relaxed);

//     let handler = SigHandler::Handler(on_sigalrm);
//     unsafe { signal::signal(Signal::SIGALRM, handler)? };

//     let _ = unistd::alarm::set(timeout_secs);
//     Ok(())
// }

pub fn async_kill(child_pid: Pid, timeout_ms: u64) -> JoinHandle<()> {
    task::spawn(async move {
        time::sleep(Duration::from_millis(timeout_ms)).await;
        let _ = send_signal(child_pid, Signal::SIGKILL);
    })
}

pub fn send_signal(pid: Pid, signal: Signal) -> nix::Result<()> {
    let result = signal::kill(pid, signal);
    debug!(
        "kill pid = {}, signal = {}, result = {:?}",
        pid, signal, result
    );
    result
}

pub fn killall(pids: &[Pid]) {
    for &pid in pids {
        let _ = send_signal(pid, Signal::SIGSTOP);
    }

    for &pid in pids {
        let _ = send_signal(pid, Signal::SIGKILL);
    }
}
