use std::hint::black_box;

use codecrafters_git::cli::{Git, GitCommand};

use iai_callgrind::{
    Callgrind, FlamegraphConfig, FlamegraphKind, LibraryBenchmarkConfig, library_benchmark,
    library_benchmark_group, main,
};

#[library_benchmark]
fn write_tree() {
    let cmd = GitCommand::WriteTree {
        path: Some("/home/aiman/Develop/exercism".into()),
    };
    let mut out = Vec::new();
    let git = black_box(Git::with_bytes_buffer(cmd, &mut out));
    black_box(git.run().expect("failed git run"));
    let out = String::from_utf8_lossy(&out);
    assert_eq!("a96b3a143adf0ca62ba774f9f3e7a20a27399898\n", out);
}

library_benchmark_group!(name = git_write_tree; benchmarks = write_tree);
main!(config = LibraryBenchmarkConfig::default()
    .tool(Callgrind::default()
        .flamegraph(FlamegraphConfig::default()
            .kind(FlamegraphKind::Differential))
    );
    library_benchmark_groups = git_write_tree
);
