# Edomata for Rust: the book

The user documentation of the Rust port, as an [mdBook](https://rust-lang.github.io/mdBook/), published at <https://beyond-scale-group.github.io/edomata/rust/book/> on every push to `main`.

```bash
cargo install mdbook
mdbook build rust/book      # HTML under rust/book/book/
mdbook serve rust/book      # live preview
```

Every Rust code block of the chapters is included from `samples/` (the `edomata-book-samples` workspace member) by anchor, so the samples are compiled and linted by `cargo clippy --workspace --all-targets --all-features` and exercised by `cargo test -p edomata-book-samples` (the PostgreSQL integration test of the testing chapter reads `DATABASE_URL`; the other samples that need a database or a broker are compiled but not run, and the broker ones are behind the `kafka` / `rabbitmq` features). The porting map and the ADRs are included from `rust/PORTING.md` and `rust/docs/adr/`.

After a build, `check_links.py` checks that every include and anchor exists and that every relative link and `#fragment` of the rendered HTML resolves (CI runs it):

```bash
cd rust
mdbook build book && python3 book/check_links.py
```
