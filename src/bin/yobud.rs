use signal_hook::{consts::SIGCHLD, iterator::Signals};
use std::{io, thread};
use yobu::daemon::server;

fn start_child_reaper() -> io::Result<()> {
    let mut signals = Signals::new([SIGCHLD])?;
    thread::spawn(move || {
        for _ in signals.forever() {
            loop {
                // SAFETY: waitpid is called from a normal thread, and the null
                // status pointer is valid because exit statuses are not needed.
                let pid = unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) };
                if pid <= 0 {
                    break;
                }
            }
        }
    });
    Ok(())
}

fn main() -> io::Result<()> {
    start_child_reaper()?;
    server::start()
}
