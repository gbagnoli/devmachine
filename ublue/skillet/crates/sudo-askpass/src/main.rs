fn main() {
    let result = std::env::current_exe()
        .map_err(skillet_sudo_askpass::Error::from)
        .and_then(|path| skillet_sudo_askpass::run(&path, &mut std::io::stdout()));
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
