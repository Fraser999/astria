//! To run all `astria-core` benchmarks, from the root of the monorepo, run:
//! ```sh
//! cargo bench --features=benchmark -qp astria-core
//! ```

fn main() {
    // Required to force the benchmark target to actually register the divan benchmark cases.
    // See https://github.com/nvzqz/divan/issues/61#issuecomment-2500002168.
    let _ = astria_core::primitive::v1::TransactionId::new([0; 32]);

    // Handle `nextest` querying the benchmark binary for tests.  Currently `divan` is incompatible
    // with `nextest`, so just report no tests available.
    // See https://github.com/nvzqz/divan/issues/43 for further details.
    let args: Vec<_> = std::env::args().collect();
    if args.contains(&"--list".to_string())
        && args.contains(&"--format".to_string())
        && args.contains(&"terse".to_string())
    {
        return;
    }
    // Run registered benchmarks.
    divan::main();
}
