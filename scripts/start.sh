#!/bin/bash

cargo run -- --listen-address /ip4/127.0.0.1/tcp/40837 \
		 --secret-key-seed 1 \
		 provide \
		 --path /home/sajmon/agentowe.txt \
		 --name awesome
