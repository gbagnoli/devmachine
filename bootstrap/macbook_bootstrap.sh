#!/bin/bash

which brew &>/dev/null || \
  usr/bin/ruby -e "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/master/install)"

APPS=(
  bash
  git
  neovim
  shellcheck
  pyenv
  rbenv
  liquidprompt
  autossh
  mosh
  fasd
  wget
  htop
  syncthing
)

CASKS=(
  caskroom/fonts/font-dejavusansmono-nerd-font
  caskroom/fonts/font-dejavusansmono-nerd-font-mono
  caskroom/fonts/font-ubuntumono-nerd-font
  caskroom/fonts/font-ubuntumono-nerd-font-mono
  caskroom/fonts/font-codenewroman-nerd-font
  caskroom/fonts/font-codenewroman-nerd-font-mono
  karabiner-elements
  spectacle
  keepassx
)

TAPS=(
  caskroom/fonts
)

SERVICES=(
  syncthing
)

tmpdir="$(mktemp -d)"
cleanup() {
  rm -rf "$tmpdir"
}
trap cleanup EXIT

for tap in "${TAPS[@]}"; do
  brew tap "$tap"
done

brew install "${APPS[@]}"
brew cask install "${CASKS[@]}"

for srv in "${SERVICES[@]}"; do
  brew services start "$srv"
done

mkdir -p "$HOME"/.local/{bin,src}
mkdir -p "$HOME"/.config/nvim/bundle
mkdir -p "$HOME/Downloads/Screenshots"

defaults write com.apple.screencapture location "$HOME/Downloads/Screenshots"
killall SystemUIServer

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

if [ ! -f "$HOME"/.local/.bashrc.local ]; then
  echo 'alias vim=nvim' >> "$HOME"/.local/.bashrc.local
fi

# templates are not really templates yet, so just download them
wget -O "$HOME"/.config/liquid.theme 'https://github.com/gbagnoli/devmachine/raw/master/site-cookbooks/user/files/default/liquid.theme'
wget -O "$HOME"/.config/liquidpromptrc 'https://github.com/gbagnoli/devmachine/raw/master/site-cookbooks/user/templates/default/liquidpromptrc.erb'
wget -O "$HOME"/.bashrc https://github.com/gbagnoli/devmachine/raw/master/site-cookbooks/user/templates/default/bashrc.erb
