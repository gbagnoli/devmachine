# Bazzite Tailscale Operator Scripts

This repository contains utility scripts for managing Tailscale connectivity and configuration, primarily focused on setting the Tailscale operator.

## Tailscale One-Shot Script (`bazzite_set_operator.sh`)

`bazzite_set_operator.sh` is a robust, root-required script designed to ensure Tailscale is connected and running before setting the designated operator for the network. It implements connection retry logic and exponential backoff to handle temporary network delays during the initial Tailscale handshake.

## Installation

To set up and install all scripts and dependencies, run the following command:

```bash
./install.sh
```

To enforce code quality using `shellcheck` before every commit, the pre-commit hook is managed externally. After cloning the repository, run the following command in the root directory to set up the symbolic link:

```bash
ln -s $(realpath git_pre_commit_hook.sh) .git/hooks/pre-commit
```