#!/usr/bin/env -S just --justfile

alias t := test

set dotenv-load := true

build: fmt
	cargo build

test:
	cargo test

run:
	cargo run

fmt:
	cargo fmt --all
	nix fmt .

clippy:
	cargo clippy --all --all-targets --all-features

# NOTE: does not currently work because Docker images are only available for OS version 2.x, but we need 3.x
runos:
	# finch vm start
	finch run -p 9200:9200 -p 9600:9600 \
		-e "discovery.type=single-node" \
		-e "OPENSEARCH_INITIAL_ADMIN_PASSWORD=$PASS" \
		-e "OPENSEARCH_JAVA_OPTS=-Dopensearch.experimental.feature.extensions.enabled=true" public.ecr.aws/opensearchproject/opensearch:2.12.0

buildos:
	cd ./resources && finch build --build-arg JDK_ARCH=x64 -t opensearchext .

# TODO: needs an entrypoint script to cd into build directory and run gradle
runosdocker:
	finch run -it -p 9200:9200 -p 9600:9600 \
		-e "discovery.type=single-node" \
		-e "OPENSEARCH_INITIAL_ADMIN_PASSWORD=$PASS" \
		-e "OPENSEARCH_JAVA_OPTS=-Dopensearch.experimental.feature.extensions.enabled=true" opensearchext /bin/bash
runosdockerarm:
	docker run -it -p 9200:9200 -p 9600:9600 \
		-e "discovery.type=single-node" \
		-e "OPENSEARCH_INITIAL_ADMIN_PASSWORD=$PASS" \
		-e "OPENSEARCH_JAVA_OPTS=-Dopensearch.experimental.feature.extensions.enabled=true" opensearchext /bin/bash

buildosarm:
	cd ./resources && docker build \
		--platform linux/arm64 \
		--build-arg JDK_ARCH=aarch64 -t opensearchext .

loadext:
	curl -XPOST "http://localhost:9200/_extensions/initialize" -H "Content-Type:application/json" --data @examples/hello/hello.json

loadext_secure:
	curl -ku "admin:$PASS" -XPOST "https://localhost:9200/_extensions/initialize" -H "Content-Type:application/json" --data @examples/hello/hello.json

getext:
	curl -ku "admin:$PASS" -XGET "https://localhost:9200/_extensions/_hello-world-rs/hello"

#vim:ft=make
