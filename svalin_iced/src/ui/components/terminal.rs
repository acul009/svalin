pub struct Terminal<B> {
    backend: B,
}

pub trait TerminalBackend {
    type Reader: tokio::io::AsyncRead;
    type Writer: tokio::io::AsyncWrite;

    fn reader(&self) -> &Self::Reader;
    fn writer(&self) -> &Self::Writer;
}
