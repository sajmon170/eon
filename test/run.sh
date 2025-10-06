#!/bin/sh

if [[ -z "$1" ]]; then
	echo "Usage: run.sh <test_suite/test.yaml>"
	exit 1
fi

docker run \
	--shm-size=1024g --security-opt seccomp=unconfined \
	--mount type=bind,ro,source=./output/release,target=/usr/local/app/eon-client \
	--mount type=bind,source=./output/shadow,target=/usr/local/app/output \
	--mount type=bind,ro,source=./scenario,target=/usr/local/app/scenario \
	sajmon/shadow bash -c "echo Hello > /usr/local/app/test.txt; shadow /usr/local/app/scenario/$1; rm -rf /usr/local/app/output/*; mv shadow.data/* /usr/local/app/output/"
