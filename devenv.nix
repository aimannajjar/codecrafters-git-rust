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
    pkgs.codecrafters-cli
  ];


  # See full reference at https://devenv.sh/reference/options/
}

