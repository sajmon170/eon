#!/bin/sh

docker buildx create --use --name insecure-builder --buildkitd-flags '--allow-insecure-entitlement security.insecure'
docker buildx build --shm-size="1024g" --allow security.insecure -t sajmon/shadow .
