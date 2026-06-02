use std::hint::black_box;

use codecrafters_git::cli::{Git, GitCommand, GitInstance};

use iai_callgrind::{
    Callgrind, FlamegraphConfig, FlamegraphKind, LibraryBenchmarkConfig, library_benchmark,
    library_benchmark_group, main,
};

#[library_benchmark]
fn write_tree() {
    let mut git = black_box(Git::<Vec<u8>>::default());
    git.set_command(GitCommand::WriteTree);
    git.take_argument("/home/aiman/Develop/exercism");
    black_box(git.run().expect("failed git run"));
    let out = String::from_utf8_lossy(git.get_out());
    assert_eq!("d529e3c4ef04b94207273fac1c3042213670cc7b\n", out);
}

library_benchmark_group!(name = git_write_tree; benchmarks = write_tree);
main!(config = LibraryBenchmarkConfig::default()
    .tool(Callgrind::default()
        .flamegraph(FlamegraphConfig::default()
            .kind(FlamegraphKind::Differential))
    );
    library_benchmark_groups = git_write_tree
);
