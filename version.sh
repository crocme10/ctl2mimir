#!/bin/sh

cat Cargo.toml | grep '^version' | grep '\([0-9]\+\.\?\)\{3\}' -o
