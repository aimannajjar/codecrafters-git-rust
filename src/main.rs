use codecrafters_git::{
    GitError,
    cli::{Git, GitCommand},
};

#[tokio::main]
async fn main() -> Result<(), GitError> {
    if cfg!(debug_assertions) {
        if std::env::args().len() == 2 {
            let mut out = Vec::new();
            let _ = std::fs::remove_dir_all("target-repo-test");
            let cmd = GitCommand::Clone {
                url: "https://github.com/codecrafters-io/git-sample-2".to_string(),
                // url: "http://192.168.80.71:3001/codecrafters/test-repo.git".to_string(),
                // url: "https://github.com/HurraTech/hagent.git".to_string(),
                dir: Some("target-repo-test".into()),
            };
            let git = Git::with_bytes_buffer(cmd, &mut out);
            git.run().await?;
            println!("{}", String::from_utf8_lossy(&out));
        } else {
            let git = Git::from_env().expect("failed to instantiate git");
            git.run().await?;
        };
        std::process::exit(0);
    }

    let git = Git::from_env().expect("failed to instantiate git");
    git.run().await?;
    Ok(())
}
