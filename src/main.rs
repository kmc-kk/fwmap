use std::process;

fn main() {
    match fwmap::cli::run(std::env::args()) {
        Ok(code) => {
            if code != 0 {
                process::exit(code);
            }
        }
        Err(err) => {
            eprintln!("error: {err}");
            process::exit(1);
        }
    }
}
