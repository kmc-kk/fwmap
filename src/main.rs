use std::process;

fn main() {
    if let Err(err) = fwmap::cli::run(std::env::args()) {
        eprintln!("error: {err}");
        process::exit(1);
    }
}
