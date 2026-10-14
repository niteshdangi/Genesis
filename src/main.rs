fn main() {
    if let Err(error) = genesis::cli::entry() {
        eprintln!("Genesis: {error}");
        std::process::exit(1);
    }
}
