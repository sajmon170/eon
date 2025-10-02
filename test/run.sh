#!/bin/sh

docker run \
	--mount type=bind,source=../target/release/eon-client,target=/usr/local/app/eon-client \
	--mount type=bind,source=.,target=/usr/local/app/data \
	sajmon/shadow shadow /usr/local/app/data/scenario/simple/simple.yaml
