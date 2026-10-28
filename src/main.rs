fn main() {
    if let Err(err) = pmvid::app::run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
