fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::args().any(|argument| argument == "--smoke") {
        match earth_two_client::physics::run_smoke() {
            Ok(position) => println!("Physics smoke passed: {position:?}"),
            Err(error) => {
                eprintln!("Physics smoke failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    #[cfg(feature = "viewer")]
    earth_two_client::viewer::run();
    #[cfg(not(feature = "viewer"))]
    {
        eprintln!("Use --smoke, or build with the viewer feature.");
        std::process::exit(2);
    }
}
