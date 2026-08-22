use std::os::unix::process::CommandExt;

fn main() {
    let mut executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("sxp: could not locate snipexpand: {error}");
            std::process::exit(127);
        }
    };
    executable.set_file_name("snipexpand");

    let error = std::process::Command::new(executable)
        .args(std::env::args_os().skip(1))
        .exec();
    eprintln!("sxp: could not launch snipexpand: {error}");
    std::process::exit(127);
}
