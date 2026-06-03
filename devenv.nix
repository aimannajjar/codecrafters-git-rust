{
  pkgs,
  lib,
  config,
  ...
}:
{
  # https://devenv.sh/languages/
  languages.rust = {
    enable = true;
    channel = "nightly";
    # By default, this will include cargo, rustc, etc.
  };

  packages = [
    pkgs.zsh
    pkgs.openssl
    pkgs.codecrafters-cli
    pkgs.cmake
  ];

  env = {
    "GIT_DIR" = "../git-gitdir";
  };

  # See full reference at https://devenv.sh/reference/options/
}

