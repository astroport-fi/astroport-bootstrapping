# Optimized builds
docker run --rm -v "$(pwd)":/code --mount type=bind,source=/.cargo/git,target=/usr/local/cargo/git --mount type=bind,source=/.cargo/registry,target=/usr/local/cargo/registry cosmwasm/workspace-optimizer:0.12.3
