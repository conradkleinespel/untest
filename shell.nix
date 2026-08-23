{pkgs ? import <nixpkgs> {}}:
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    rustup
  ];
  buildInputs = with pkgs; [
    podman
  ];
  shellHook = ''
    git config set core.hooksPath githooks

    rustup default stable
    rustup component add rust-src
    rustup target add x86_64-unknown-linux-gnu
  '';
  GIT_COMMIT_MSG_SCOPES = "lib devenv docs misc";
}
