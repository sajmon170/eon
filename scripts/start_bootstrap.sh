#!/bin/sh

cargo run -- --secret-key-seed 1 \
		 --bootstrap-mode \
		 provide \
		 --path /home/sajmon/pomiary.txt \
		 --name pomiary
