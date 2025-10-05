# syntax=docker/dockerfile:1-labs

FROM debian:12
WORKDIR /usr/local/app

RUN apt-get update

RUN apt-get install -y \
    curl \
    cmake \
    findutils \
    libclang-dev \
    libc-dbg \
    libglib2.0-0 \
    libglib2.0-dev \
    make \
    netbase \
    python3 \
    python3-networkx \
    xz-utils \
    util-linux \
    gcc \
    g++

COPY shadow ./shadow
WORKDIR /usr/local/app/shadow

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

RUN ./setup build --clean --test
RUN --security=insecure ./setup test
RUN ./setup install

ENV PATH="/root/.local/bin:${PATH}"
