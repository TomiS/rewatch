# Debian "slim-bookworm" based image
FROM rust:1.85-bullseye AS build

# Define username as arg. Weirdly this is necessary.
ARG USERNAME=${USERNAME}

# Run initial apt-get update
RUN apt-get update --fix-missing

# Install sudo to be able to change file permissions in the end (hack)
RUN apt-get install -y sudo

# Create docker user with same name as local user and a group with username, also add to sudoers to enable sudo
# https://stackoverflow.com/questions/68937464/running-devcontainer-as-local-user-instead-of-root
# https://github.com/microsoft/vscode-remote-release/issues/6834#issuecomment-1157911582
# Docker default user and group id
ARG PUID=1000
ARG PGID=1000
RUN set -x && groupadd --gid $PGID ${USERNAME} && \
  useradd --no-log-init --gid $PGID --create-home --shell /bin/bash --uid $PUID ${USERNAME} --groups sudo && \
  echo "${USERNAME}:${USERNAME}" | chpasswd && \
  echo "${USERNAME} ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/${USERNAME}

# Install C/Gcc compiler (for Rust)
RUN apt-get install -y build-essential

# Install Node/JS runtime env
RUN apt-get install -y curl gnupg gnupg2 gnupg1
RUN curl -sS https://dl.yarnpkg.com/debian/pubkey.gpg | apt-key add -
RUN echo "deb https://dl.yarnpkg.com/debian/ stable main" | tee /etc/apt/sources.list.d/yarn.list > /dev/null
RUN apt-get update && apt-get install -y --no-install-recommends yarn

RUN curl -sL https://deb.nodesource.com/setup_18.x  | bash -
RUN apt-get -y install nodejs


#####################################################################
# FROM THIS POINT ON, INSTALL DEPENDENCIES AS LOCAL USER (Not root)
#####################################################################
USER "${USERNAME}"

# Install Rust and Cargo
ENV RUST_VERSION=1.85.0
# RUN curl -y --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y

ENV PATH="/home/${USERNAME}/.cargo/bin:${PATH}"

RUN rustup component add rustfmt