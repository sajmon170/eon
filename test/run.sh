#!/bin/sh

docker run \
	--shm-size=1024g --security-opt seccomp=unconfined \
	--mount type=bind,ro,source=./output/release,target=/usr/local/app/eon-client \
	--mount type=bind,source=./output/shadow,target=/usr/local/app/output \
	--mount type=bind,ro,source=./scenario,target=/usr/local/app/scenario \
	sajmon/shadow bash -c "shadow /usr/local/app/scenario/simple/simple.yaml && mv shadow.data/* /usr/local/app/output/"
