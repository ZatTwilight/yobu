use yobu::daemon::server;

fn main() {
    println!("Starting YOBU broker POC...");
    if std::env::var("YOBU_SOCKET").is_err() {
        eprintln!("Warning: YOBU_SOCKET not set, using temp dev socket");
        let path = "/tmp/yobu_test.sock";
        unsafe {
            std::env::set_var("YOBU_SOCKET", path);
        }
    }
    server::start().expect("failed to start server");
}
