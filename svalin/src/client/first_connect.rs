pub enum FirstConnect {
    Init(Init),
    Login(Login),
}

struct Init {
    rpc: svalin_rpc::Client,
}

struct Login {
    rpc: svalin_rpc::Client,
}
