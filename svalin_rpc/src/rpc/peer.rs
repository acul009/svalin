use svalin_pki::Certificate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Peer {
    Anonymous,
    Certificate(Certificate),
}
