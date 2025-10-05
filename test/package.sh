#!/bin/sh

docker run \
	--shm-size=1024g --security-opt seccomp=unconfined \
	--mount type=bind,ro,source=../,target=/usr/local/app/build \
	--mount type=bind,source=./output,target=/usr/local/app/build/target \
    sajmon/shadow bash -c "cd /usr/local/app/build && cargo build --release"
