#!/bin/bash

exec cargo build -r
exec sudo mv ../target/release/snip /usr/bin
