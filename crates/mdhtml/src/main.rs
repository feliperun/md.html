use std::ffi::OsString;

fn main() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();

    match mdhtml::run_cli(args) {
        Ok(text) => print!("{text}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
