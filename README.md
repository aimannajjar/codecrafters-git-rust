### Git Implementation
This is an incomplete git client implementation in rust (solving codecrafters exercises). It features:
1. Smart HTTP protocol parsing uses `winnow`
2. Implements basic commands such as `commit`, `hash-object`, `cat-file`, `ls-tree`, `write-tree`
3. Implements first stage of `clone` (i.e. upload-pack, over http transport, smart protocol)

I've employed several Rust concepts attempting an elegant design. I liked the how the struct-encoded stateful implementation turned out in `protocol.rs` (compile-time state checking!) and the usage of `winnow` for parsing smart http responses made the code easy. However, there is much room for optimization for both file and memory operations.
