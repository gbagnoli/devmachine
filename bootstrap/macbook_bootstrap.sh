#!/bin/bash

set -euo pipefail

which brew &>/dev/null || \
  usr/bin/ruby -e "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install)"

function fixperm() {
  while true; do
    sudo chown -R giacomo /usr/local/bin /usr/local/lib /usr/local/sbin
    sleep 30 || break
  done
}
fixperm &
FIXPERM=$!

tmpdir="$(mktemp -d)"
cleanup() {
  kill "$FIXPERM"
  rm -rf "$tmpdir"
}
trap cleanup EXIT


pushd "$(dirname "${0%/*}")" > /dev/null 2>&1 || exit 1
brew tap Homebrew/bundle
brew bundle --file macos_brewfile

mkdir -p "$HOME"/.local/{bin,src}
mkdir -p "$HOME"/.config/nvim/bundle
mkdir -p "$HOME/Downloads/Screenshots"

if [[ "$(defaults read com.apple.screencapture location)" != "$HOME/Downloads/Screenshots" ]]; then
  echo "Setting the default location of screenshots to $HOME/Downloads/Screenshots"
  defaults write com.apple.screencapture location "$HOME/Downloads/Screenshots"
  killall SystemUIServer
fi

# dotfiles!
cd "$HOME/.local/src" || exit 1
if [ ! -d dotfiles ]; then
  git clone https://github.com/gbagnoli/dotfiles.git
fi

dotfiles="$HOME/.local/src/dotfiles"
ln -sf "${dotfiles}/gitconfig" "$HOME"/.gitconfig
ln -sf "${dotfiles}/gitignore" "$HOME/.gitignore_global"
ln -sf "${dotfiles}/inputrc" "$HOME/.inputrc"
ln -sf "${dotfiles}/tmux.conf" "$HOME/.tmux.conf"
[ -d ~/.vim ] && rm -rf .vim
ln -sf "$HOME/.config/nvim" "${HOME}/.vim"
ln -sf "${dotfiles}/vim/vimrc" "$HOME/.config/nvim/init.vim"

if [ ! -d "$HOME"/.config/nvim/bundle/Vundle.vim ]; then
  cd "$HOME"/.config/nvim/bundle/ || exit 1
  git clone https://github.com/VundleVim/Vundle.vim.git
  nvim +PluginInstall +qall!
fi

if [ ! -d "$HOME"/.local/src/autoenv ]; then
  cd "$HOME"/.local/src || exit 1
  git clone git://github.com/kennethreitz/autoenv.git
fi

# gpicsync
if [ ! -d "$HOME/.local/src/GPicSync" ]; then
  cd "$HOME/.local/src" || exit 1
  git clone https://github.com/metadirective/GPicSync.git
fi

cat  > "$HOME/.local/bin/gpicsync" << 'EOF'
#!/bin/bash
exec python2 "$HOME/.local/src/GPicSync/src/gpicsync.py" "$@"
EOF

chmod +x "$HOME/.local/bin/gpicsync"

if [ ! -f "$HOME"/.local/.bashrc.local ]; then
  echo 'alias vim=nvim' >> "$HOME"/.local/.bashrc.local
fi

# templates are not really templates yet, so just download them
wget -nc -O "$HOME"/.config/liquid.theme 'https://github.com/gbagnoli/devmachine/raw/master/site-cookbooks/user/files/default/liquid.theme'
wget -nc -O "$HOME"/.config/liquidpromptrc 'https://github.com/gbagnoli/devmachine/raw/master/site-cookbooks/user/templates/default/liquidpromptrc.erb'
wget -nc -O "$HOME"/.bashrc https://github.com/gbagnoli/devmachine/raw/master/site-cookbooks/user/templates/default/bashrc.erb
chmod +x "$HOME/.local/bin/photo_process.sh"
