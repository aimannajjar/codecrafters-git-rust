use codecrafters_git::cli::{Git, GitCommand};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("write_tree_new", |b| {
        b.iter(|| {
            let cmd = GitCommand::WriteTree {
                path: Some("/home/aiman/Develop/exercism".into()),
            };
            let mut out = Vec::new();
            let git = black_box(Git::with_bytes_buffer(cmd, &mut out));
            black_box(git.run().expect("failed git run"))
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
