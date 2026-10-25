use std::io;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut external = rekall_dev::shell;
    let code = rekall_dev::run(&args, &root, &mut external, &mut io::stderr());
    std::process::exit(i32::from(code));
}
