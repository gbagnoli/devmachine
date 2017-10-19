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
  gpsbabel
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
exec python "$HOME/.local/src/GPicSync/src/gpicsync.py" "$@"
EOF

chmod +x "$HOME/.local/bin/gpicsync"

if [ ! -f "$HOME"/.local/.bashrc.local ]; then
  echo 'alias vim=nvim' >> "$HOME"/.local/.bashrc.local
fi

# templates are not really templates yet, so just download them
wget -nc -O "$HOME"/.config/liquid.theme 'https://github.com/gbagnoli/devmachine/raw/master/site-cookbooks/user/files/default/liquid.theme'
wget -nc -O "$HOME"/.config/liquidpromptrc 'https://github.com/gbagnoli/devmachine/raw/master/site-cookbooks/user/templates/default/liquidpromptrc.erb'
wget -nc -O "$HOME"/.bashrc https://github.com/gbagnoli/devmachine/raw/master/site-cookbooks/user/templates/default/bashrc.erb

wget -nc -O "$HOME/.local/bin/photo_process.sh" https://gist.githubusercontent.com/gbagnoli/28565417cfb732cbd2df784819a7fcb0/raw/2e8967cba2d9eb19f9b0fd554b0a91eece075858/photo_process.sh
chmod +x "$HOME/.local/bin/photo_process.sh"
