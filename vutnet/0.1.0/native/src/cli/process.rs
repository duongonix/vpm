use std::{
    io,
    process::{ChildStderr, ChildStdout, Command, ExitStatus},
};

use process_wrap::std::{ChildWrapper, CommandWrap};

#[cfg(windows)]
use process_wrap::std::JobObject;
#[cfg(unix)]
use process_wrap::std::ProcessGroup;

pub struct ChildProcess {
    child: Box<dyn ChildWrapper>,
}

impl ChildProcess {
    pub fn spawn(command: Command) -> io::Result<Self> {
        let mut wrapped = CommandWrap::from(command);
        #[cfg(unix)]
        wrapped.wrap(ProcessGroup::leader());
        #[cfg(windows)]
        wrapped.wrap(JobObject);
        Ok(Self {
            child: wrapped.spawn()?,
        })
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout().take()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr().take()
    }

    pub fn try_terminate(&mut self) -> io::Result<()> {
        if self.child.try_wait()?.is_some() {
            return Ok(());
        }
        self.child.kill()
    }

    pub fn terminate(&mut self) {
        let _ = self.try_terminate();
    }
}

impl Drop for ChildProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[cfg(test)]
mod tests {
    use std::{
        thread,
        time::{Duration, Instant},
    };

    use super::*;

    #[test]
    fn wrapped_process_exits_and_process_tree_terminates() {
        let mut child = ChildProcess::spawn(short_command()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "child process did not exit");
            thread::sleep(Duration::from_millis(10));
        }

        let mut child = ChildProcess::spawn(long_command()).unwrap();
        child.terminate();
        assert!(child.try_wait().unwrap().is_some());
    }

    fn short_command() -> Command {
        if cfg!(windows) {
            let mut command = Command::new("cmd.exe");
            command.args(["/D", "/S", "/C", "exit 0"]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", "exit 0"]);
            command
        }
    }

    fn long_command() -> Command {
        if cfg!(windows) {
            let mut command = Command::new("cmd.exe");
            command.args(["/D", "/S", "/C", "ping -n 30 127.0.0.1 >NUL"]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", "sleep 30"]);
            command
        }
    }
}
