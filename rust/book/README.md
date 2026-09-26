# Edomata for Rust: the book

The user documentation of the Rust port, as an [mdBook](https://rust-lang.github.io/mdBook/).

```bash
cargo install mdbook
mdbook build rust/book      # HTML under rust/book/book/
mdbook serve rust/book      # live preview
```

Every code block of the tutorials (except the `AuthPolicy` trait quoted in the SaaS chapter) is included from `samples/` (the `edomata-book-samples` workspace member), so the samples are compiled and linted by `cargo clippy --workspace --all-targets` and, when they need no database, exercised by `cargo test -p edomata-book-samples`. The porting map and the ADRs are included from `rust/PORTING.md` and `rust/docs/adr/`.
