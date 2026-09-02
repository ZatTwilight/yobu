use std::io;
use yobu::daemon::server;

fn main() -> io::Result<()> {
    // Child owners reap their own processes so launch helpers can inspect status.
    server::start()
}
