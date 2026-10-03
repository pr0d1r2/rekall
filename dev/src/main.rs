use std::io;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut external = |command: &str| {
        std::process::Command::new("sh")
            .args(["-c", command])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .map_err(|error| error.to_string())
    };
    let code = rekall_dev::run(&args, &root, &mut external, &mut io::stderr());
    std::process::exit(i32::from(code));
}
