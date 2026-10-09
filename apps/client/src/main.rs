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
    {
        #[cfg(not(target_arch = "wasm32"))]
        let foundation = std::env::args().any(|a| a == "--foundation");
        #[cfg(target_arch = "wasm32")]
        let foundation = web_sys::window()
            .and_then(|w| w.location().search().ok())
            .and_then(|s| web_sys::UrlSearchParams::new_with_str(&s).ok())
            .is_some_and(|p| p.has("foundation"));
        if foundation {
            earth_two_client::viewer::run();
        } else {
            earth_two_client::game::run();
        }
    }
    #[cfg(not(feature = "viewer"))]
    {
        eprintln!("Use --smoke, or build with the viewer feature.");
        std::process::exit(2);
    }
}
