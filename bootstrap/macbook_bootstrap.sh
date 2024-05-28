#!/bin/bash

set -euo pipefail

if ! which brew &>/dev/null ; then
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
fi

tmpdir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT


pushd "$(dirname "${0%/*}")" > /dev/null 2>&1 || exit 1
brew bundle --file macos_brewfile

mkdir -p "$HOME"/.local/{bin,src}
mkdir -p "$HOME"/.vim
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
ln -sf "${dotfiles}/vim/vimrc" "$HOME/.vimrc"

if [ ! -d "$HOME"/.vim/bundle/Vundle.vim ]; then
  git clone https://github.com/VundleVim/Vundle.vim.git ~/.vim/bundle/Vundle.vim
  vim +PluginInstall +qall!
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

# templates are not really templates yet, so just download them
# wget -nc -O "$HOME"/.config/liquid.theme 'https://github.com/gbagnoli/devmachine/raw/master/site-cookbooks/user/files/default/liquid.theme'
wget -nc -O "$HOME"/.config/liquidpromptrc 'https://github.com/gbagnoli/devmachine/raw/master/site-cookbooks/user/templates/default/liquidpromptrc.erb'
wget -nc -O "$HOME"/.bashrc https://github.com/gbagnoli/devmachine/raw/master/site-cookbooks/user/templates/default/bashrc.erb
chmod +x "$HOME/.local/bin/photo_process.sh"
