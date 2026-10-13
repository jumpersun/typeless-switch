//! Bounded waits for short-lived OS helpers, never for the Typeless process itself.
use std::{
    process::{Child, Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CommandError {
    Failed,
    TimedOut,
    CleanupFailed,
}

#[derive(Default)]
pub(crate) struct Runner {
    pending: Mutex<Vec<Child>>,
}

impl Runner {
    pub(crate) fn ensure_idle(&self) -> Result<(), CommandError> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| CommandError::CleanupFailed)?;
        reap(&mut pending)
    }

    pub(crate) fn run(&self, command: &mut Command, deadline: Instant) -> Result<(), CommandError> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| CommandError::CleanupFailed)?;
        reap(&mut pending)?;
        run(command, deadline, &mut pending)
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        if let Ok(pending) = self.pending.get_mut() {
            for child in pending {
                let _ = child.kill();
                let _ = child.try_wait();
            }
        }
    }
}

fn reap(pending: &mut Vec<Child>) -> Result<(), CommandError> {
    pending.retain_mut(|child| !matches!(child.try_wait(), Ok(Some(_))));
    if pending.is_empty() {
        Ok(())
    } else {
        Err(CommandError::CleanupFailed)
    }
}

fn run(
    command: &mut Command,
    deadline: Instant,
    pending: &mut Vec<Child>,
) -> Result<(), CommandError> {
    // A shared deadline also bounds multiple quit helpers on Windows.
    if Instant::now() >= deadline {
        return Err(CommandError::TimedOut);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| CommandError::Failed)?;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(CommandError::Failed)
                }
            }
            Err(_) => return Err(terminate(child, CommandError::Failed, pending)),
            Ok(None) => (),
        }
        if Instant::now() >= deadline {
            return Err(terminate(child, CommandError::TimedOut, pending));
        }
        thread::sleep(
            Duration::from_millis(20).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

fn terminate(mut child: Child, reason: CommandError, pending: &mut Vec<Child>) -> CommandError {
    // Only the helper we spawned is terminated. Never kill Typeless or a process tree.
    let _ = child.kill();
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return reason,
            Err(_) => break,
            Ok(None) if Instant::now() >= deadline => break,
            Ok(None) => thread::sleep(Duration::from_millis(10)),
        }
    }
    // Keep the handle so restoration and further helpers stay blocked until it exits.
    // No unbounded wait may hold the account mutex or the desktop's exit guard.
    pending.push(child);
    CommandError::CleanupFailed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs, thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    fn helper(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        let name = format!(
            "{}::helper_process",
            module_path!().split_once("::").unwrap().1
        );
        command
            .args(["--ignored", "--exact", &name])
            .env("TYPELESS_COMMAND_TEST", mode);
        command
    }

    fn marker() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "typeless-command-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    #[ignore = "runs only as an isolated child process"]
    fn helper_process() {
        match std::env::var("TYPELESS_COMMAND_TEST").unwrap().as_str() {
            "success" => (),
            "failure" => std::process::exit(7),
            "idle" => thread::sleep(Duration::from_millis(500)),
            "delayed" => {
                thread::sleep(Duration::from_millis(500));
                fs::write(
                    std::env::var_os("TYPELESS_COMMAND_MARKER").unwrap(),
                    b"late action",
                )
                .unwrap();
            }
            _ => panic!("unknown test helper"),
        }
    }

    #[test]
    fn returns_success_and_nonzero_exit() {
        assert_eq!(
            Runner::default().run(
                &mut helper("success"),
                Instant::now() + Duration::from_secs(2)
            ),
            Ok(())
        );
        assert_eq!(
            Runner::default().run(
                &mut helper("failure"),
                Instant::now() + Duration::from_secs(2)
            ),
            Err(CommandError::Failed)
        );
    }

    #[test]
    fn timeout_stops_helper_before_it_can_act_later() {
        let marker = marker();
        let started = Instant::now();
        let result = Runner::default().run(
            helper("delayed").env("TYPELESS_COMMAND_MARKER", &marker),
            started + Duration::from_millis(100),
        );
        let elapsed = started.elapsed();
        thread::sleep(Duration::from_millis(550));
        let acted = marker.exists();
        let _ = fs::remove_file(&marker);
        assert_eq!(result, Err(CommandError::TimedOut));
        assert!(
            elapsed < Duration::from_millis(500),
            "helper was not bounded: {elapsed:?}"
        );
        assert!(!acted, "timed-out helper performed a late action");
    }

    #[test]
    fn exhausted_deadline_does_not_start_another_helper() {
        assert_eq!(
            Runner::default().run(
                &mut helper("success"),
                Instant::now() - Duration::from_millis(1)
            ),
            Err(CommandError::TimedOut)
        );
    }

    #[test]
    fn unreaped_helper_blocks_recovery_and_next_command_until_it_exits() {
        let runner = Runner::default();
        let child = helper("idle")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        runner.pending.lock().unwrap().push(child);
        assert_eq!(runner.ensure_idle(), Err(CommandError::CleanupFailed));
        assert_eq!(
            runner.run(
                &mut helper("success"),
                Instant::now() + Duration::from_secs(2)
            ),
            Err(CommandError::CleanupFailed)
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        while runner.ensure_idle().is_err() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(runner.ensure_idle(), Ok(()));
        assert_eq!(runner.run(&mut helper("success"), deadline), Ok(()));
    }
}
