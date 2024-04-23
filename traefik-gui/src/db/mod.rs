use once_cell::sync::Lazy;

pub static DB: Lazy<marmelade::DB> =
    Lazy::new(|| marmelade::DB::open("./traefik-gui.jammdb").expect("Could not open Database"));
